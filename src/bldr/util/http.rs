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

use hyper;
use hyper::client::Client;
use std::io::{Read, Write, BufWriter};
use std::fs::{self, File};
use error::{BldrResult, BldrError};

pub fn download(status: &str, url: &str, path: &str) -> BldrResult<String> {
    let mut client = Client::new();
    debug!("Making request to url {}", url);
    let mut res = try!(client.get(url).send());
    debug!("Response: {:?}", res);
    let length = res.headers.get::<hyper::header::ContentLength>()
        .map_or("Unknown".to_string(), |v| format!("{}", v));
    // Here is a moment where you can really like Rust. We create
    // a file, wrap it in a BufWriter - which understands how to
    // safely batch writes into large buffer sizes on the heap,
    // saving us the tax of frequent system calls. We then do
    // what we would do in C - create a buffer of bytes, then
    // read into that buffer, and write out to the other side.
    //
    // Under the hood, Hyper uses the "BufReader" to implement
    // reading the request body - so on both ends, we are getting
    // free buffering on the heap, using our stack buffer just to
    // shuttle back and forth. This is essentially with the "tee"
    // function does in the stdlib, but with error handling that
    // a generic "tee" can't really provide easily.
    //
    // What you can't see is this - the compiler helped with
    // making sure all the edge cases of the pattern were covered,
    // and even though its a trivial case, it was pretty great.
    let file_name = try!(file_name(url));
    let tempfile = format!("{}/{}.tmp", path, file_name);
    let finalfile = format!("{}/{}", path, file_name);
    let f = try!(File::create(&tempfile));
    let mut writer = BufWriter::new(&f);
    let mut written: i64 = 0;
    let mut buf = [0u8; 100000]; // Our byte buffer
    loop {
        let len = try!(res.read(&mut buf)); // Raise IO errors
        match len {
            0 => { // 0 == EOF, so stop writing and finish progress
                progress(status, written, &length, true);
                break;
            },
            _ => { // Write the buffer to the BufWriter on the Heap
                let bytes_written = try!(writer.write(&buf[0 .. len]));
                if bytes_written == 0 {
                    return Err(BldrError::WriteSyncFailed);
                }
                written = written + (bytes_written as i64);
                progress(status, written, &length, false);
            }
        };
    }
    try!(fs::rename(&tempfile, &finalfile));
    Ok(finalfile)
}

fn file_name(url: &str) -> BldrResult<&str> {
    let result = try!(url.split("/").last().ok_or(BldrError::CannotParseFileName));
    Ok(result)
}

fn progress(status: &str, written: i64, length: &str, finished: bool) {
    let progress = format!("   {}: {}/{}", status, written, length);
    print!("{}", from_char(progress.len(), '\x08'));
    if finished {
        println!("{}", progress);
    } else {
        print!("{}", progress);
    }
}

fn from_char(length: usize, ch: char) -> String {
    if length == 0 {
        return String::new()
    }

    let mut buf = String::new();
    buf.push(ch);
    let size = buf.len() * length;
    buf.reserve(size);
    for _ in 1 .. length {
        buf.push(ch)
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::{file_name, from_char};

    #[test]
    fn file_name_returns_filename_from_url() {
        assert_eq!(
            "rise.bldr".to_string(),
            file_name("http://example.com/done_asking/rise.bldr").unwrap()
        );
    }

    #[test]
    fn file_name_returns_filename_from_just_name() {
        assert_eq!("rise.bldr", file_name("rise.bldr").unwrap());
    }

    #[test]
    fn from_char_returns_the_correct_string() {
        assert_eq!("xxxx", from_char(4, 'x'));
    }
}