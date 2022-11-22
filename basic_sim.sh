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
#                Bank::new(8, "fish")
#                    .name(3, "banana")
#                    .name(5, "apple")
#                    .hog(1, "breath", Direction::OutputHigh),
#            )
#            .with_bank(
#                Bank::new(12, "babel")
#                    .name(3, "piñata")
#                    .name(5, "piggly")
#                    .name(7, "apple")
#                    .hog(2, "hogster", Direction::Input),
#                    .hog(8, "breath", Direction::OutputLow),
#            )
#            .live()

mkdir /sys/kernel/config/gpio-sim/basic

mkdir /sys/kernel/config/gpio-sim/basic/bank0
echo "fish" > /sys/kernel/config/gpio-sim/basic/bank0/label
echo 8 > /sys/kernel/config/gpio-sim/basic/bank0/num_lines
mkdir /sys/kernel/config/gpio-sim/basic/bank0/line3
echo "banana" > /sys/kernel/config/gpio-sim/basic/bank0/line3/name
mkdir /sys/kernel/config/gpio-sim/basic/bank0/line5
echo "apple" > /sys/kernel/config/gpio-sim/basic/bank0/line5/name
mkdir -p /sys/kernel/config/gpio-sim/basic/bank0/line1/hog
echo "breath" > /sys/kernel/config/gpio-sim/basic/bank0/line1/hog/name
echo "output-high" > /sys/kernel/config/gpio-sim/basic/bank0/line1/hog/direction

mkdir /sys/kernel/config/gpio-sim/basic/bank1
echo "babel" > /sys/kernel/config/gpio-sim/basic/bank1/label
echo 12 > /sys/kernel/config/gpio-sim/basic/bank1/num_lines
mkdir /sys/kernel/config/gpio-sim/basic/bank1/line3
echo "piñata" > /sys/kernel/config/gpio-sim/basic/bank1/line3/name
mkdir /sys/kernel/config/gpio-sim/basic/bank1/line5
echo "piggly" > /sys/kernel/config/gpio-sim/basic/bank1/line5/name
mkdir /sys/kernel/config/gpio-sim/basic/bank1/line7
echo "apple" > /sys/kernel/config/gpio-sim/basic/bank1/line7/name
mkdir -p /sys/kernel/config/gpio-sim/basic/bank1/line2/hog
echo "hogster" > /sys/kernel/config/gpio-sim/basic/bank1/line2/hog/name
echo "input" > /sys/kernel/config/gpio-sim/basic/bank1/line2/hog/direction
mkdir -p /sys/kernel/config/gpio-sim/basic/bank1/line8/hog
echo "breath" > /sys/kernel/config/gpio-sim/basic/bank1/line8/hog/name
echo "output-low" > /sys/kernel/config/gpio-sim/basic/bank1/line8/hog/direction

echo 1 > /sys/kernel/config/gpio-sim/basic/live

