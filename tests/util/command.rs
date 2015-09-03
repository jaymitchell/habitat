//
// Copyright:: Copyright (c) 2015 Chef Software, Inc.
// License:: Apache License, Version 2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use std::io::prelude::*;
use std::io;
use std::process::{Command, Child, Stdio, ExitStatus};
use std::fmt;
use std::error::Error;
use std::result;
use std::ffi::OsStr;
use std::path::Path;
use std::thread;

use util;

pub struct Cmd {
    pub child: Option<Child>,
    pub status: Option<ExitStatus>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

impl Cmd {
    pub fn kill(&mut self) -> &Self {
        match self.child {
            Some(ref mut child) => {
                child.kill().unwrap_or_else(|x| panic!("{:?}", x));
            },
            None => panic!("Cannot kill a child that does not exist - you have probably called wait_with_output already"),
        }
        self
    }

    pub fn stdout(&self) -> &str {
        match self.stdout {
            Some(ref stdout) => stdout,
            None => panic!("No stdout available - process needs a wait")
        }
    }

    pub fn stderr(&self) -> &str {
        match self.stderr {
            Some(ref stderr) => stderr,
            None => panic!("No stderr available - process needs a wait")
        }
    }

    pub fn status(&self) -> &ExitStatus {
        match self.status {
            Some(ref status) => status,
            None => panic!("No status available - process needs a wait or kill")
        }
    }

    pub fn wait_with_output(&mut self) -> &Self {
        // The child is unavailable for more calls after this
        let child = self.child.take().unwrap();

        let output = match child.wait_with_output() {
            Ok(output) => output,
            Err(e) => panic!("{:?}", e)
        };
        self.status = Some(output.status);
        let stdout = String::from_utf8(output.stdout).unwrap_or_else(|x| panic!("{:?}", x));
        let stderr = String::from_utf8(output.stderr).unwrap_or_else(|x| panic!("{:?}", x));
        self.stdout = Some(stdout);
        self.stderr = Some(stderr);
        self
    }
}

#[derive(Debug)]
pub enum CmdError {
    Io(io::Error),
}

pub type CmdResult<T> = result::Result<T, CmdError>;

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CmdError::Io(ref err) => err.fmt(f),
        }
    }
}

impl Error for CmdError {
    fn description(&self) -> &str {
        match *self {
            CmdError::Io(ref err) => err.description(),
        }
    }
}

impl From<io::Error> for CmdError {
    fn from(err: io::Error) -> CmdError {
        CmdError::Io(err)
    }
}

pub fn command(cmd: &str, args: &[&str]) -> Command {
    println!("{}: Running: cmd: {} {:?}", thread::current().name().unwrap_or("main"), cmd, args);
    let mut command = Command::new(cmd);
    command.args(args);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command
}

pub fn spawn(mut command: Command) -> CmdResult<Cmd> {
    let child = try!(command.spawn());
    Ok(Cmd{ child: Some(child), status: None, stdout: None, stderr: None })
}

pub fn run(cmd: &str, args: &[&str]) -> CmdResult<Cmd> {
    let command = command(cmd, args);
    spawn(command)
}

pub fn bldr_build<P: AsRef<Path>>(cwd: P) -> CmdResult<Cmd> {
    let bldr_build = util::path::bldr_build();
    let mut command = command(&bldr_build, &["Bldrfile"]);
    command.env("BLDR_FROM", util::path::bldr());
    command.current_dir(cwd);
    spawn(command)
}

pub fn bldr(args: &[&str]) -> CmdResult<Cmd> {
    let bldr = util::path::bldr();
    let mut command = command(&bldr, args);
    spawn(command)
}
