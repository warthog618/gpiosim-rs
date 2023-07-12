#!/bin/env sh
# SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
#
# SPDX-License-Identifier: Apache-2.0 OR MIT

# An example of creating a basic sim directly using the configfs.
#
# This is the equivalent of
#         let sim = gpiosim::builder()
#            .with_name("basic")
#            .with_bank(
#                Bank::new(8, "fruit")
#                    .name(3, "banana")
#                    .name(5, "apple")
#                    .hog(1, "hog1", Direction::OutputHigh),
#            )
#            .with_bank(
#                Bank::new(12, "vegatable")
#                    .name(3, "arugula")
#                    .name(5, "broccoli")
#                    .name(7, "celery")
#                    .hog(2, "hog2", Direction::Input),
#                    .hog(8, "hog3", Direction::OutputLow),
#            )
#            .live()

mkdir /sys/kernel/config/gpio-sim/basic

mkdir /sys/kernel/config/gpio-sim/basic/bank0
echo "fruit" > /sys/kernel/config/gpio-sim/basic/bank0/label
echo 8 > /sys/kernel/config/gpio-sim/basic/bank0/num_lines
mkdir /sys/kernel/config/gpio-sim/basic/bank0/line3
echo "banana" > /sys/kernel/config/gpio-sim/basic/bank0/line3/name
mkdir /sys/kernel/config/gpio-sim/basic/bank0/line5
echo "apple" > /sys/kernel/config/gpio-sim/basic/bank0/line5/name
mkdir -p /sys/kernel/config/gpio-sim/basic/bank0/line1/hog
echo "hog1" > /sys/kernel/config/gpio-sim/basic/bank0/line1/hog/name
echo "output-high" > /sys/kernel/config/gpio-sim/basic/bank0/line1/hog/direction

mkdir /sys/kernel/config/gpio-sim/basic/bank1
echo "vegetable" > /sys/kernel/config/gpio-sim/basic/bank1/label
echo 12 > /sys/kernel/config/gpio-sim/basic/bank1/num_lines
mkdir /sys/kernel/config/gpio-sim/basic/bank1/line3
echo "arugula" > /sys/kernel/config/gpio-sim/basic/bank1/line3/name
mkdir /sys/kernel/config/gpio-sim/basic/bank1/line5
echo "broccoli" > /sys/kernel/config/gpio-sim/basic/bank1/line5/name
mkdir /sys/kernel/config/gpio-sim/basic/bank1/line7
echo "celery" > /sys/kernel/config/gpio-sim/basic/bank1/line7/name
mkdir -p /sys/kernel/config/gpio-sim/basic/bank1/line2/hog
echo "hog2" > /sys/kernel/config/gpio-sim/basic/bank1/line2/hog/name
echo "input" > /sys/kernel/config/gpio-sim/basic/bank1/line2/hog/direction
mkdir -p /sys/kernel/config/gpio-sim/basic/bank1/line8/hog
echo "hog3" > /sys/kernel/config/gpio-sim/basic/bank1/line8/hog/name
echo "output-low" > /sys/kernel/config/gpio-sim/basic/bank1/line8/hog/direction

echo 1 > /sys/kernel/config/gpio-sim/basic/live

