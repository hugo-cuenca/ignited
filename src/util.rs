use nix::sys::utsname::uname;

pub fn get_booted_kernel_ver() -> String {
    uname().release().to_string()
}
