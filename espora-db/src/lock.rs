use std::os::fd::RawFd;

pub struct LockHandle {
    pub(crate) fd: RawFd,
}

impl Drop for LockHandle {
    fn drop(&mut self) {
        unsafe { libc::flock(self.fd, libc::LOCK_UN) };
    }
}
