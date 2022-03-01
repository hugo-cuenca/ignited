//! (Linux) device manager based on `uevent` netlink socket.

use crate::{common::ThreadHandle, early_logging::KConsole, PROGRAM_NAME};
use mio::{Token, Waker};
use precisej_printable_errno::{printable_error, PrintableErrno};
use std::{
    sync::{mpsc::channel, Arc},
    thread,
};

/// udev thread event loop waker.
const UDEV_THREAD_WAKE_TOKEN: Token = Token(20);

/// udev thread `uevent` netlink socket.
const UDEV_THREAD_UEVENT_NL_TOKEN: Token = Token(21);

mod listener {
    use super::{UDEV_THREAD_UEVENT_NL_TOKEN, UDEV_THREAD_WAKE_TOKEN};
    use crate::{early_logging::KConsole, PROGRAM_NAME};
    use kobject_uevent::{ActionType, UEvent};
    use mio::{Events, Interest, Poll, Waker};
    use netlink_sys::{protocols::NETLINK_KOBJECT_UEVENT, Socket, SocketAddr};
    use precisej_printable_errno::{printable_error, PrintableErrno};
    use std::{
        process::id as getpid,
        sync::{mpsc::Sender, Arc},
        thread,
    };

    /// Function called when the listener thread is spawned.
    pub(super) fn spawn(
        main_waker: Arc<Waker>,
        tx_udev_waker: Sender<Result<Arc<Waker>, PrintableErrno<String>>>,
    ) {
        // KConsole has been successfully opened before, so this should never fail.
        let mut kcon = KConsole::new().unwrap();

        let mut evloop = match Poll::new().map_err(|io| {
            printable_error(
                PROGRAM_NAME,
                format!("error while setting up udev event loop: {}", io),
            )
        }) {
            Ok(poll) => poll,
            Err(e) => {
                let _ = tx_udev_waker.send(Err(e));
                return;
            }
        };
        let mut evs = Events::with_capacity(3);
        let udev_waker = match Waker::new(evloop.registry(), UDEV_THREAD_WAKE_TOKEN).map_err(|io| {
            printable_error(
                PROGRAM_NAME,
                format!("error while setting up udev waker: {}", io),
            )
        }) {
            Ok(waker) => Arc::new(waker),
            Err(e) => {
                let _ = tx_udev_waker.send(Err(e));
                return;
            }
        };
        if tx_udev_waker.send(Ok(Arc::clone(&udev_waker))).is_err() {
            return;
        };
        drop(tx_udev_waker);

        // uevent socket
        let mut uevent_socket = Socket::new(NETLINK_KOBJECT_UEVENT).unwrap();
        uevent_socket.set_non_blocking(true).unwrap();
        let uevent_sa = SocketAddr::new(getpid(), 1);
        let mut uevent_buf = vec![0; 1024 * 8];
        uevent_socket.bind(&uevent_sa).unwrap();
        evloop
            .registry()
            .register(
                &mut uevent_socket,
                UDEV_THREAD_UEVENT_NL_TOKEN,
                Interest::READABLE,
            )
            .unwrap();

        loop {
            match evloop.poll(&mut evs, None) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                result => result.unwrap(),
            };

            let mut quit_thread = false;
            for ev in evs.iter() {
                match ev.token() {
                    UDEV_THREAD_UEVENT_NL_TOKEN => {
                        let packet_size = match uevent_socket.recv(&mut uevent_buf, 0) {
                            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                                // EOF means closed netlink socket. We should return from
                                // this thread
                                quit_thread = true;
                                continue;
                            }
                            result => result.unwrap(),
                        };
                        let uevent = match UEvent::from_netlink_packet(&uevent_buf[..packet_size]) {
                            Ok(uevent) => uevent,
                            Err(_) => {
                                // Error means EOF, which means closed netlink socket. We
                                // should return from this thread
                                quit_thread = true;
                                continue;
                            }
                        };

                        kdebug!(kcon, "udev event {:?}", uevent);

                        // spawn thread for each uevent
                        let main_waker = Arc::clone(&main_waker);
                        thread::spawn(move || handle_uevent(main_waker, uevent));
                    }
                    UDEV_THREAD_WAKE_TOKEN => {
                        // root is already mounted, we can exit
                        return;
                    }
                    _ => {}
                }
            }
            if quit_thread {
                return;
            }
        }
    }

    fn handle_uevent(main_waker: Arc<Waker>, uevent: UEvent) {
        // KConsole has been successfully opened before, so this should never fail.
        let mut kcon = KConsole::new().unwrap();

        if let Some(modalias) = uevent.env.get("MODALIAS") {
            handle_uevent_load_modalias(&mut kcon, modalias);
        } else if uevent.subsystem == "block" {
            handle_uevent_block_device(&mut kcon, uevent);
        } else if uevent.subsystem == "net" {
            handle_uevent_network(&mut kcon, uevent);
        } else if uevent.subsystem == "hidraw" && uevent.action == ActionType::Add {
            todo!();
        }
    }

    fn handle_uevent_load_modalias(kcon: &mut KConsole, modalias: &str) {
        todo!()
    }

    fn handle_uevent_block_device(kcon: &mut KConsole, uevent: UEvent) {
        todo!()
    }

    fn handle_uevent_network(kcon: &mut KConsole, uevent: UEvent) {
        todo!()
    }
}

/// `uevent` listener.
#[derive(Debug)]
pub struct UdevListener(ThreadHandle);
impl UdevListener {
    /// Construct a new listener which will notify when `/system_root` is mounted.
    pub fn listen(main_waker: &Arc<Waker>) -> Result<Self, PrintableErrno<String>> {
        let main_waker = Arc::clone(main_waker);
        let (tx_udev_waker, rx_udev_waker) = channel();

        let handle = thread::spawn(move || listener::spawn(main_waker, tx_udev_waker));
        let udev_waker = rx_udev_waker.recv().map_err(|e| {
            printable_error(
                PROGRAM_NAME,
                format!("error while spawning udev thread: {}", e),
            )
        })??;
        Ok(Self(ThreadHandle::new("udev", handle, udev_waker)))
    }

    /// Stop the `uevent` listener and cleanup.
    pub fn stop(self, kcon: &mut KConsole) {
        self.0.join_now(kcon);
    }
}
