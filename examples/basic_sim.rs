// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use gpiosim::{Bank, Direction};
use std::result::Result;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    gpiosim::builder()
        .with_name("basic")
        .with_bank(
            Bank::new(8, "fruit")
                .name(3, "banana")
                .name(5, "apple")
                .hog(1, "hog1", Direction::OutputHigh),
        )
        .with_bank(
            Bank::new(12, "vegatable")
                .name(3, "arugula")
                .name(5, "broccoli")
                .name(7, "celery")
                .hog(2, "hog2", Direction::Input)
                .hog(8, "hog3", Direction::OutputLow),
        )
        .live()?;
    Ok(())
}
