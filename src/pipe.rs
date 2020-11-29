//! Abstraction over UNIX-pipe specific for the use case here.

use crate::error::UECOError;
use crate::libc_util::{libc_ret_to_result, LibcSyscall};
use crate::{OCatchStrategy};

/// Convenient wrapper around the pipes that we
/// need for the desired output catch strategy.
#[derive(Debug)]
pub enum CatchPipes {
    Combined(Pipe),
    Separately{stdout: Pipe, stderr: Pipe}
}

impl CatchPipes {
    pub fn new(strategy: OCatchStrategy) -> Result<Self, UECOError> {
        match strategy {
            OCatchStrategy::StdCombined => {
                Ok(
                    CatchPipes::Combined(Pipe::new()?)
                )
            }
            OCatchStrategy::StdSeparately => {
                Ok(
                    CatchPipes::Separately{
                        stdout: Pipe::new()?,
                        stderr: Pipe::new()?,
                    }
                )
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum PipeEnd {
    Read = 0,
    Write = 1,
}

#[derive(Debug)]
pub struct Pipe {
    end: Option<PipeEnd>,
    read_fd: libc::c_int,
    write_fd: libc::c_int,
}

impl Pipe {

    pub(crate) fn new() -> Result<Self, UECOError> {
        let mut fds: [libc::c_int; 2] = [0, 0];
        let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
        libc_ret_to_result(ret, LibcSyscall::Pipe)?;

        trace!("pipe created successfully");

        let pipe = Self {
                end: None,
                read_fd: fds[PipeEnd::Read as usize],
                write_fd: fds[PipeEnd::Write as usize],
        };

        Ok(pipe)
    }

    pub(crate) fn mark_as_parent_process(&mut self) -> Result<(), UECOError> {
        trace!("pipe marked as read end");
        self.end.replace(PipeEnd::Read);
        self.close_fd(self.write_fd)
    }

    pub(crate) fn mark_as_child_process(&mut self) -> Result<(), UECOError> {
        trace!("pipe marked as write end");
        self.end.replace(PipeEnd::Write);
        self.close_fd(self.read_fd)
    }

    /// Try to read the next line from the read end of the pipe.
    /// Returns ERR if a syscall failed. Returns OK(None) if
    /// EOF was reached. Returns (Ok(Some(String)) if a new line
    /// was read.
    pub(crate) fn read_line(&self) -> Result<Option<String>, UECOError> {
        if *self.end.as_ref().expect("Kind of Pipeend must be specified at this point") != PipeEnd::Read {
            return Err(UECOError::PipeNotMarkedAsReadEnd);
        }

        let mut chars = Vec::new();

        loop {
            // read from file descriptor byte by byte (each iteration results in a syscall)
            let char = self.read_char()?;
            if char.is_none() {
                return Ok(None); // EOF
            }
            let char = char.unwrap();
            if char == '\n' {
                trace!("newline (\\n) found");
                break
            }
            chars.push(char);
        }

        let string = chars.into_iter().collect::<String>();
        Ok(
            Some(string)
        )
    }

    /// Reads a single char from the read end of the pipe (Some(char)) or EOF (None).
    fn read_char(&self) -> Result<Option<char>, UECOError> {
        const BUF_LEN: usize = 1; // Todo this is not efficient
        let mut buf: [char; BUF_LEN] = ['\0'];
        let buf_ptr = buf.as_mut_ptr() as * mut libc::c_void;
        let ret = unsafe { libc::read(self.read_fd, buf_ptr, BUF_LEN) };

        // check error and unwrap
        libc_ret_to_result(ret as i32, LibcSyscall::Read)?;

        // EOF
        if ret == 0 {
            Ok(None)
        } else {
            let char = buf[0];
            Ok(Some(char))
        }
    }

    fn close_fd(&self, fd: libc::c_int) -> Result<(), UECOError> {
        let ret = unsafe { libc::close(fd) };
        libc_ret_to_result(ret, LibcSyscall::Close)
    }

    pub fn read_fd(&self) -> i32 {
        self.read_fd
    }

    pub fn write_fd(&self) -> i32 {
        self.write_fd
    }

    /*pub fn end(&self) -> &Option<PipeEnd> {
        &self.end
    }*/
}
