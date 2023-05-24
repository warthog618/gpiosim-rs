#!/bin/env sh
# SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
#
# SPDX-License-Identifier: Apache-2.0 OR MIT

# A helper to remove any orphaned gpio-sims from the system.
# This should only be necessary if a test was killed abnormally
# preventing it from cleaning up the sims it created, or if you
# created a sim using basic_sim.sh.

find /sys/kernel/config/gpio-sim -type d -name hog -print0 2>/dev/null | xargs -0 -r rmdir
find /sys/kernel/config/gpio-sim -type d -name "line*" -print0  2>/dev/null | xargs -0 -r rmdir
find /sys/kernel/config/gpio-sim -type d -name "bank*" -print0 2>/dev/null | xargs -0 -r rmdir
rmdir /sys/kernel/config/gpio-sim/*

