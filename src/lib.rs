// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A library for creating and controlling GPIO simulators for testing users of
//! the Linux GPIO uAPI (both v1 and v2).
//!
//! The simulators are provided by the Linux **gpio-sim** kernel module and require a
//! recent kernel (v5.19 or later) built with **CONFIG_GPIO_SIM**.
//!
//! Simulators ([`Sim`]) contain one or more chips, each with a collection of lines being
//! simulated. The [`Builder`] is responsible for constructing the [`Sim`] and taking it live.
//! Configuring a simulator involves adding [`Bank`]s, representing the
//! chip, to the [`Builder`].
//!
//! Once live, the [`Chip`] exposes lines which may be manipulated to drive the
//! GPIO uAPI from the kernel side.
//! For input lines, applying a pull using [`Chip.set_pull`] and related
//! methods controls the level of the simulated line.  For output lines,
//! [`Chip.get_level`] returns the level the simulated line is being driven to.
//!
//! For simple tests that only require lines on a single chip, the [`Simpleton`]
//! provides a simplified interface.
//!
//! Configuring a simulator involves *configfs*, and manipulating the chips once live
//! involves *sysfs*, so root permissions are typically required to run a simulator.
//!
//! ## Example Usage
//!
//! Creating a simulator with two chips, with 8 and 42 lines respectively, each with
//! several named lines and a hogged line:
//!
//! ```no_run
//! # use gpiosim::Result;
//! # fn main() -> Result<()> {
//! use gpiosim::{Bank, Direction, Level};
//!
//! let sim = gpiosim::builder()
//!     .with_name("some unique name")
//!     .with_bank(
//!         Bank::new(8, "left")
//!             .name(3, "LED0")
//!             .name(5, "BUTTON1")
//!             .hog(2, "hogster", Direction::OutputLow)
//!         )
//!     .with_bank(
//!         Bank::new(42, "right")
//!             .name(3, "BUTTON2")
//!             .name(4, "LED2")
//!             .hog(7, "hogster", Direction::OutputHigh),
//!     )
//!     .live()?;
//!
//! let chips = sim.chips();
//! let c = &chips[0];
//! c.set_pull(5, Level::High);
//! let level = c.get_level(3)?;
//! # Ok(())
//! # }
//! ```
//!
//! Use a [`Simpleton`] to create a single chip simulator with 12 lines, for where multiple chips or
//! named lines are not required:
//!
//! ```no_run
//! # use gpiosim::Result;
//! # fn main() -> Result<()> {
//! use gpiosim::{Level, Simpleton};
//!
//! let s = Simpleton::new(12);
//! let c = s.chip();
//! c.set_pull(5, Level::High);
//! c.set_pull(6, Level::Low);
//! let level = c.get_level(3)?;
//! # Ok(())
//! # }
//! ```
//!
//! [`Chip.set_pull`]: struct.Chip.html#method.set_pull
//! [`Chip.get_level`]: struct.Chip.html#method.get_level

use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::hash::{BuildHasherDefault, Hasher};
use std::io::prelude::*;
use std::io::BufReader;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread::sleep;
use std::time::Duration;

/// A live simulator of one or more chips.
#[derive(Debug, Eq, PartialEq)]
pub struct Sim {
    /// The name of the simulator in configfs and sysfs space.
    name: String,

    /// The details of the chips being simulated.
    chips: Vec<Chip>,

    /// Path to the gpio-sim in configfs.
    dir: PathBuf,
}

impl Sim {
    /// The details of the chips being simulated.
    pub fn chips(&self) -> &[Chip] {
        self.chips.as_slice()
    }

    /// The name of the simulator in configfs and sysfs space.
    pub fn name(&self) -> &str {
        &self.name
    }

    fn live(&mut self) -> Result<()> {
        self.setup_configfs()?;
        write_attr(&self.dir, "live", "1")?;
        self.read_attrs()
    }

    fn cleanup_configfs(&mut self) {
        if !self.dir.exists() {
            return;
        }
        let _ = write_attr(&self.dir, "live", "0");
        for (i, c) in self.chips.iter().enumerate() {
            let bank = format!("bank{i}");
            let bank_dir = self.dir.join(bank);
            if !bank_dir.exists() {
                continue;
            }
            for offset in c.cfg.hogs.keys() {
                let line_dir = bank_dir.join(format!("line{offset}"));
                let hog_dir = line_dir.join("hog");
                let _ = fs::remove_dir(hog_dir);
                let _ = fs::remove_dir(line_dir);
            }
            for offset in c.cfg.names.keys() {
                let line_dir = bank_dir.join(format!("line{offset}"));
                let _ = fs::remove_dir(line_dir);
            }
            let _ = fs::remove_dir(bank_dir);
        }
        let _ = fs::remove_dir(&self.dir);
        while self.dir.exists() {}
    }

    fn setup_configfs(&mut self) -> Result<()> {
        for (i, c) in self.chips.iter().enumerate() {
            let bank_dir = self.dir.join(format!("bank{i}"));
            fs::create_dir(&bank_dir)?;
            write_attr(&bank_dir, "label", c.cfg.label.as_bytes())?;
            write_attr(&bank_dir, "num_lines", format!("{}", c.cfg.num_lines))?;

            for (offset, name) in &c.cfg.names {
                let line_dir = bank_dir.join(format!("line{offset}"));
                fs::create_dir(&line_dir)?;
                write_attr(&line_dir, "name", name.as_bytes())?;
            }
            for (offset, hog) in &c.cfg.hogs {
                let line_dir = bank_dir.join(format!("line{offset}"));
                if !line_dir.exists() {
                    fs::create_dir(&line_dir)?;
                }
                let hog_dir = line_dir.join("hog");
                fs::create_dir(&hog_dir)?;
                write_attr(&hog_dir, "name", hog.consumer.as_bytes())?;
                write_attr(&hog_dir, "direction", hog.direction.as_str())?;
            }
        }
        Ok(())
    }

    fn read_attrs(&mut self) -> Result<()> {
        let dev_name = read_attr(&self.dir, "dev_name")?;
        for (i, c) in self.chips.iter_mut().enumerate() {
            let bank_dir = self.dir.join(format!("bank{i}"));
            let chip_name = read_attr(&bank_dir, "chip_name")?;
            c.dev_path = "/dev".into();
            c.dev_path.push(&chip_name);
            // Flag anyone (e.g. Raspberry Pi) fixing their backward compatibility gpiochip renumbering
            // issues using symlinks that then conflict with dynamically created gpiochips.
            // This can result in the gpio-sim users inadvertently driving ACTUAL hardware,
            // which is obviously something that can not be allowed, so bail out.
            let metadata = fs::symlink_metadata(&c.dev_path)?;
            assert!(
                !metadata.is_symlink(),
                "A symlink ({}) is masking device {}.  Please remove the symlink.",
                &c.dev_path.display(),
                &chip_name,
            );
            let mut sysfs_path = PathBuf::from("/sys/devices/platform");
            sysfs_path.push(&dev_name);
            sysfs_path.push(&chip_name);
            c.sysfs_path = sysfs_path;
            c.chip_name = chip_name;
            c.dev_name.clone_from(&dev_name);
        }
        Ok(())
    }
}

impl Drop for Sim {
    fn drop(&mut self) {
        self.cleanup_configfs();
    }
}

/// A live simulated chip.
#[derive(Debug)]
pub struct Chip {
    /// The path to the chip in /dev
    ///
    /// e.g. `/dev/gpiopchip0`
    dev_path: PathBuf,

    /// The name of the gpiochip in /dev and sysfs.
    ///
    /// e.g. `gpiochip0`
    pub chip_name: String,

    /// The name of the device in sysfs.
    ///
    /// e.g. `gpio-sim.0`
    pub dev_name: String,

    /// The path to the chip in /sys/device/platform.
    ///
    /// e.g. `/sys/devices/platform/gpio-sim.0`
    sysfs_path: PathBuf,

    /// The configuration for the chip.
    cfg: Bank,
}

impl Chip {
    /// The configuration for the chip
    pub fn config(&self) -> &Bank {
        &self.cfg
    }

    /// The path to the chip in /dev
    ///
    /// e.g. `/dev/gpiopchip0`
    pub fn dev_path(&self) -> &PathBuf {
        &self.dev_path
    }

    /// Pull a line to simulate the line being externally driven.
    pub fn set_pull(&self, offset: Offset, pull: Level) -> Result<()> {
        let value = match pull {
            Level::Low => "pull-down",
            Level::High => "pull-up",
        };
        let path = format!("sim_gpio{offset}/pull");
        fs::write(self.sysfs_path.join(path), value).map_err(Error::IoError)
    }

    /// Pull a line up to simulate the line being externally driven high.
    pub fn pullup(&self, offset: Offset) -> Result<()> {
        self.set_pull(offset, Level::High)
    }

    /// Pull a line down to simulate the line being externally driven low.
    pub fn pulldown(&self, offset: Offset) -> Result<()> {
        self.set_pull(offset, Level::Low)
    }

    /// Toggle the pull on a line.
    pub fn toggle(&self, offset: Offset) -> Result<Level> {
        let value = match self.get_pull(offset)? {
            Level::High => Level::Low,
            Level::Low => Level::High,
        };
        self.set_pull(offset, value)?;
        Ok(value)
    }

    fn get_attr(&self, offset: Offset, attr: &str) -> Result<String> {
        let path = format!("sim_gpio{offset}/{attr}");
        fs::read_to_string(self.sysfs_path.join(path))
            .map(|s| s.trim().to_string())
            .map_err(Error::IoError)
    }

    /// Get the current state of the simulated external pull on a line.
    pub fn get_pull(&self, offset: Offset) -> Result<Level> {
        let pull = self.get_attr(offset, "pull")?;
        match pull.as_str() {
            "pull-down" => Ok(Level::Low),
            "pull-up" => Ok(Level::High),
            _ => Err(Error::UnexpectedValue(pull)),
        }
    }

    /// Get the current output value for a simulated output line.
    pub fn get_level(&self, offset: Offset) -> Result<Level> {
        let val = self.get_attr(offset, "value")?;
        match val.as_str() {
            "0" => Ok(Level::Low),
            "1" => Ok(Level::High),
            _ => Err(Error::UnexpectedValue(val)),
        }
    }
}
impl PartialEq for Chip {
    fn eq(&self, other: &Self) -> bool {
        self.dev_path == other.dev_path
    }
}
impl Eq for Chip {}

/// Start building a GPIO simulator.
pub fn builder() -> Builder {
    Builder::default()
}

/// A basic single bank/chip sim.
///
/// This is sufficient for tests that do not require named lines, hogged lines
/// or multiple chips.
pub struct Simpleton {
    pub sim: Sim,
}

impl Simpleton {
    /// Build a basic single bank sim and take it live.
    ///
    /// This is sufficient for tests that do not require named lines, hogged lines
    /// or multiple chips.
    ///
    ///
    pub fn new(num_lines: u32) -> Simpleton {
        Simpleton {
            sim: builder()
                .with_bank(&Bank::new(num_lines, "simpleton"))
                .live()
                .unwrap(),
        }
    }

    /// Return the only chip simulated by the Simpleton.
    pub fn chip(&self) -> &Chip {
        &self.sim.chips[0]
    }

    /// The configuration for the Simpleton chip
    pub fn config(&self) -> &Bank {
        &self.sim.chips[0].cfg
    }

    /// Return path to the chip in /dev.
    pub fn dev_path(&self) -> &PathBuf {
        &self.sim.chips[0].dev_path
    }

    /// Pull a line to simulate the line being externally driven.
    pub fn set_pull(&self, offset: Offset, pull: Level) -> Result<()> {
        self.sim.chips[0].set_pull(offset, pull)
    }

    /// Pull a line up to simulate the line being externally driven high.
    pub fn pullup(&self, offset: Offset) -> Result<()> {
        self.set_pull(offset, Level::High)
    }

    /// Pull a line down to simulate the line being externally driven low.
    pub fn pulldown(&self, offset: Offset) -> Result<()> {
        self.set_pull(offset, Level::Low)
    }

    /// Toggle the pull on a line.
    pub fn toggle(&self, offset: Offset) -> Result<Level> {
        self.sim.chips[0].toggle(offset)
    }

    /// Get the current state of the simulated external pull on a line.
    pub fn get_pull(&self, offset: Offset) -> Result<Level> {
        self.sim.chips[0].get_pull(offset)
    }

    /// Get the current output value for a simulated output line.
    pub fn get_level(&self, offset: Offset) -> Result<Level> {
        self.sim.chips[0].get_level(offset)
    }
}

/// A builder of simulators.
///
/// Collects the configuration for the simulator, and then creates
/// the simulator when taken live.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Builder {
    /// The name for the simulator in the configfs space.
    ///
    /// If None when [`live`] is called then a unique name is generated.
    ///
    /// [`live`]: Builder::live
    pub name: Option<String>,

    /// The details of the banks to be simulated.
    ///
    /// Each bank becomes a chip when the simulator goes live.
    pub banks: Vec<Bank>,
}

impl Builder {
    /// A convenience function to add a bank to the configuration.
    pub fn with_bank(&mut self, bank: &Bank) -> &mut Self {
        self.banks.push(bank.clone());
        self
    }

    /// A convenience function to specify the name for the simulator.
    ///
    /// The name must be unique or going live will fail.
    pub fn with_name<N: Into<String>>(&mut self, name: N) -> &mut Self {
        self.name = Some(name.into());
        self
    }

    /// Take the builder config live and return the created simulator.
    ///
    /// If no name has been provided for the builder then one is generated
    /// in the format `<app>-p<pid>-<N>` where:
    ///  - the app name is drawn from `argv[0]` of the executable
    ///  - pid is the process id
    ///  - N is a counter of sims taken live by this process, starting at 0
    pub fn live(&mut self) -> Result<Sim> {
        let name = match &self.name {
            Some(n) => n.clone(),
            None => default_name(),
        };
        let sim_dir = find_configfs()?.join(&name);
        if sim_dir.exists() {
            return Err(Error::SimulatorExists(name));
        }
        fs::create_dir(&sim_dir)?;

        let mut sim = Sim {
            name,
            chips: Vec::new(),
            dir: sim_dir,
        };
        for b in &self.banks {
            sim.chips.push(Chip {
                cfg: b.clone(),
                dev_path: PathBuf::default(),
                chip_name: String::default(),
                dev_name: String::default(),
                sysfs_path: PathBuf::default(),
            })
        }
        sim.live()?;

        Ok(sim)
    }
}

/// The offset of a line on a chip.
pub type Offset = u32;

/// A map from offset to T.
pub type OffsetMap<T> = HashMap<Offset, T, BuildHasherDefault<OffsetHasher>>;

/// A simple identity hasher for maps using Offsets as keys.
#[derive(Default)]
pub struct OffsetHasher(u64);

impl Hasher for OffsetHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, _: &[u8]) {
        panic!("OffsetHasher key must be u32")
    }

    fn write_u32(&mut self, n: u32) {
        self.0 = n.into()
    }
}

/// The configuration for a single simulated chip.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Bank {
    /// The number of lines simulated by this bank.
    pub num_lines: u32,

    /// The label of the chip.
    pub label: String,

    /// Lines assigned a name.
    pub names: OffsetMap<String>,

    /// Lines that appear to be already in use by some other entity.
    pub hogs: OffsetMap<Hog>,
}

impl Bank {
    /// Basic constructor.
    pub fn new<N: Into<String>>(num_lines: u32, label: N) -> Bank {
        Bank {
            num_lines,
            label: label.into(),
            names: OffsetMap::default(),
            hogs: OffsetMap::default(),
        }
    }

    /// Assign a name to a line on the chip.
    pub fn name<N: Into<String>>(&mut self, offset: Offset, name: N) -> &mut Self {
        self.names.insert(offset, name.into());
        self
    }

    /// Remove the name from a line.
    pub fn unname(&mut self, offset: Offset) -> &mut Self {
        self.names.remove(&offset);
        self
    }

    /// Add a hog on a line on the chip.
    ///
    /// A "hog" simulates some other user holding the line.
    ///
    /// If a line is hogged it is not available to be requested via the uAPI.
    pub fn hog<N: Into<String>>(
        &mut self,
        offset: Offset,
        consumer: N,
        direction: Direction,
    ) -> &mut Self {
        self.hogs.insert(
            offset,
            Hog {
                direction,
                consumer: consumer.into(),
            },
        );
        self
    }

    /// Unhog a line on the chip.
    pub fn unhog(&mut self, offset: Offset) -> &mut Self {
        self.hogs.remove(&offset);
        self
    }
}

/// The configuration for a hogged line.
///
/// A "hogged" line appears to be already requested by a consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hog {
    /// The name of the consumer that appears to be using the line.
    pub consumer: String,

    /// The requested direction for the hogged line, and if an
    /// output then the pull.
    pub direction: Direction,
}

/// The direction, and for outputs the pulled value, of a hogged line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// Hogged line is requested as an input.
    Input,

    /// Hogged line is requested as an output pulled low.
    OutputLow,

    /// Hogged line is requested as an output pulled high.
    OutputHigh,
}

impl Direction {
    fn as_str(&self) -> &'static str {
        match self {
            Direction::Input => "input",
            Direction::OutputHigh => "output-high",
            Direction::OutputLow => "output-low",
        }
    }
}

/// The physical value of a line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    /// The line is  physically high.
    High,

    /// The line is physically low.
    Low,
}

impl Level {
    /// Toggle the level between high and low.
    pub fn toggle(&self) -> Level {
        match self {
            Level::High => Level::Low,
            Level::Low => Level::High,
        }
    }
}

/// Create a unique, but predictable, name for the simulator.
///
/// The name format is `<app>-p<pid>-<N>[-<instance>]`
/// where:
///   - the app name provided by the caller
///   - pid is the process id
///   - N is a counter of names generated, starting at 0
///   - instance is optionally provided by the caller
pub fn unique_name(app: &str, instance: Option<&str>) -> String {
    static NAME_COUNT: AtomicU32 = AtomicU32::new(0);

    let mut name = format!(
        "{}-p{}-{}",
        app,
        process::id(),
        NAME_COUNT.fetch_add(1, Ordering::Relaxed)
    );
    if let Some(i) = instance {
        name += "-";
        name += i;
    }
    name
}

// Helper to write to simulator configuration files.
fn write_attr<D: AsRef<[u8]>>(p: &Path, file: &str, data: D) -> Result<()> {
    let path = p.join(file);
    fs::write(path, data).map_err(Error::IoError)
}

// Helper to read from simulator attribute files.
fn read_attr(p: &Path, file: &str) -> Result<String> {
    let path = p.join(file);
    fs::read_to_string(path)
        .map(|s| s.trim().to_string())
        .map_err(Error::IoError)
}

fn app_name() -> String {
    if let Some(app) = env::args_os().next() {
        if let Some(path) = Path::new(app.as_os_str()).file_name() {
            if let Some(app) = path.to_str() {
                return app.into();
            }
        }
    }
    "gpiosim".into()
}

fn default_name() -> String {
    unique_name(&app_name(), None)
}

fn configfs_mountpoint() -> Option<PathBuf> {
    if let Ok(f) = File::open("/proc/mounts") {
        let r = BufReader::new(f);
        for line in r.lines().map_while(|x| x.ok()) {
            let words: Vec<&str> = line.split_ascii_whitespace().collect();
            if words.len() >= 6 && words[2] == "configfs" {
                return Some(PathBuf::from(words[1]));
            }
        }
    }
    None
}

// check if configfs is mounted, and if so where.
fn find_configfs() -> Result<PathBuf> {
    // Assume default location for starters
    let path: PathBuf = "/sys/kernel/config/gpio-sim".into();
    if path.exists() {
        return Ok(path);
    }
    // Perhaps gpio-sim module is not loaded - so load it
    let output = process::Command::new("modprobe")
        .arg("gpio-sim")
        .output()
        .map_err(|e| Error::CommandError("modprobe".into(), Box::new(e)))?;
    if !output.status.success() {
        return Err(Error::ModuleLoadError(OsString::from_vec(output.stderr)));
    }
    for _ in 0..10 {
        if path.exists() {
            return Ok(path);
        }
        // Loading gpio-sim should mount configfs, but maybe it isn't in the
        // standard location, so check mounts...
        if let Some(mut cfgfs) = configfs_mountpoint() {
            cfgfs.push("gpio-sim");
            if path.exists() {
                return Ok(cfgfs);
            }
        }
        sleep(Duration::from_millis(100));
    }
    Err(Error::ConfigfsNotFound)
}

/// The result for [`gpiosim`] functions.
///
/// [`gpiosim`]: crate
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by [`gpiosim`] functions.
///
/// [`gpiosim`]: crate
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Could not find the configfs mountpoint.
    #[error("Could not find configsfs")]
    ConfigfsNotFound,

    /// An error detected while loading the gpio-sim kernel module.
    #[error("Could not load gpio-sim module: {0:?}")]
    ModuleLoadError(OsString),

    /// Attempt to take a simulator live with a name of an active simulator.
    #[error("Simulator with name {0:?} already exists")]
    SimulatorExists(String),

    /// An unexpected value was read from a configfs or sysfs attribute file.
    #[error("Read unexpected attr value {0:?}")]
    UnexpectedValue(String),

    /// An IO error detected while accessing a configfs or sysfs attribute file
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    /// An error detected while executing an external command.
    #[error("Command {0} returned error {1}")]
    CommandError(String, Box<dyn std::error::Error>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::*;

    #[test]
    fn unique_name_default() {
        let name = unique_name("my_app", None);
        assert!(name.starts_with("my_app-p"));
    }

    #[test]
    fn unique_name_explicit() {
        let name = unique_name("my_app", Some("test2"));
        assert!(name.starts_with("my_app-p"));
        assert!(name.ends_with("-test2"));
    }

    #[test]
    fn bank_constructor_default() {
        let c = Bank::default();
        assert_eq!(c.num_lines, 0);
        assert!(c.label.is_empty());
        assert_eq!(c.names.len(), 0);
        assert_eq!(c.hogs.len(), 0);
    }

    #[test]
    fn bank_name() {
        let mut c = Bank::default();
        c.name(3, "pinata");
        assert_eq!(c.names.len(), 1);
        assert_eq!(c.names[&3], "pinata");
        c.name(3, "pineapple");
        assert_eq!(c.names.len(), 1);
        assert_eq!(c.names[&3], "pineapple");
        c.name(0, "nada");
        assert_eq!(c.names.len(), 2);
        assert_eq!(c.names[&0], "nada");
    }

    #[test]
    fn bank_unname() {
        let mut c = Bank::default();
        c.name(3, "pinata");
        c.name(0, "nada");
        assert_eq!(c.names.len(), 2);
        c.unname(3);
        assert!(!c.names.contains_key(&3));
        assert_eq!(c.names.len(), 1);
        assert_eq!(c.names[&0], "nada");
    }

    #[test]
    fn bank_hog() {
        let mut c = Bank::default();
        c.hog(3, "pinata", Direction::Input);
        assert_eq!(c.hogs.len(), 1);
        c.hog(2, "piggly", Direction::OutputLow);
        assert_eq!(c.hogs.len(), 2);
        c.hog(1, "wiggly", Direction::OutputHigh);
        assert_eq!(c.hogs.len(), 3);
        assert_eq!(c.hogs[&3].consumer, "pinata");
        assert_eq!(c.hogs[&2].consumer, "piggly");
        assert_eq!(c.hogs[&1].consumer, "wiggly");
        assert_eq!(c.hogs[&3].direction, Direction::Input);
        assert_eq!(c.hogs[&2].direction, Direction::OutputLow);
        assert_eq!(c.hogs[&1].direction, Direction::OutputHigh);
        // overwrite
        c.hog(2, "wiggly", Direction::OutputHigh);
        assert_eq!(c.hogs[&2].consumer, "wiggly");
        assert_eq!(c.hogs[&2].direction, Direction::OutputHigh);
        assert_eq!(c.hogs.len(), 3);
    }

    #[test]
    fn bank_unhog() {
        let mut c = Bank::default();
        c.hog(3, "pinata", Direction::Input);
        c.hog(2, "piggly", Direction::OutputLow);
        c.hog(1, "wiggly", Direction::OutputHigh);
        assert_eq!(c.hogs.len(), 3);
        c.unhog(2);
        assert_eq!(c.hogs.len(), 2);
        assert!(!c.hogs.contains_key(&2));
        assert_eq!(c.hogs[&3].consumer, "pinata");
        assert_eq!(c.hogs[&1].consumer, "wiggly");
        assert_eq!(c.hogs[&3].direction, Direction::Input);
        assert_eq!(c.hogs[&1].direction, Direction::OutputHigh);
    }

    #[test]
    fn builder_with_bank() {
        let mut builder = builder();
        builder
            .with_bank(
                Bank::new(8, "fish")
                    .name(3, "banana")
                    .name(5, "apple")
                    .hog(5, "breath", Input),
            )
            .with_bank(
                Bank::new(42, "babel")
                    .name(3, "piñata")
                    .hog(2, "hogster", OutputHigh)
                    .hog(7, "hogster", OutputLow),
            );
        assert_eq!(builder.banks.len(), 2);
        assert_eq!(builder.banks[0].num_lines, 8);
        assert_eq!(builder.banks[0].names.len(), 2);
        assert_eq!(builder.banks[0].hogs.len(), 1);
        assert_eq!(builder.banks[1].num_lines, 42);
        assert_eq!(builder.banks[1].names.len(), 1);
        assert_eq!(builder.banks[1].hogs.len(), 2);
    }

    #[test]
    fn builder_with_name() {
        let mut builder = builder();
        assert!(builder.name.is_none());
        builder.with_name("banana");
        assert!(builder.name.is_some());
        assert_eq!(builder.name.unwrap(), "banana");
    }
}
