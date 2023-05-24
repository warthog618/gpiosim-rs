// SPDX-FileCopyrightText: 2022 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Live tests require the gpio-sim kernel module and gpiocdev to provide the
// interface from the kernel/gpiolib side.

mod simpleton {
    use gpiocdev::{chip, line};
    use gpiocdev::request::Request;
    use gpiosim::Simpleton;

    #[test]
    fn goes_live() {
        let s = Simpleton::new(12);
        assert_eq!(s.sim.chips().len(), 1);
        let c = s.chip();
        assert_eq!(c.cfg.num_lines, 12);
        assert_eq!(c.cfg.label, "simpleton");

        let cdevc = chip::Chip::from_path(c.dev_path());
        assert!(cdevc.is_ok());
        let cdevc = cdevc.unwrap();
        let info = cdevc.info();
        assert!(info.is_ok());
        let info = info.unwrap();
        let xinfo = chip::Info {
            name: String::from(&c.chip_name),
            label: "simpleton".into(),
            num_lines: 12,
        };
        assert_eq!(info, xinfo);
    }


    #[test]
    fn pull() {
        let s = Simpleton::new(8);

        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(5)
            .as_input()
            .request();
        assert!(req.is_ok());
        let req = req.unwrap();

        assert_eq!(s.get_pull(5).unwrap(), gpiosim::Level::Low);
        assert_eq!(req.value(5).unwrap(), line::Value::Inactive);

        assert!(s.pullup(5).is_ok());
        assert_eq!(s.get_pull(5).unwrap(), gpiosim::Level::High);
        assert_eq!(req.value(5).unwrap(), line::Value::Active);

        assert!(s.pulldown(5).is_ok());
        assert_eq!(s.get_pull(5).unwrap(), gpiosim::Level::Low);
        assert_eq!(req.value(5).unwrap(), line::Value::Inactive);

        assert!(s.set_pull(5, gpiosim::Level::High).is_ok());
        assert_eq!(s.get_pull(5).unwrap(), gpiosim::Level::High);
        assert_eq!(req.value(5).unwrap(), line::Value::Active);

        assert!(s.set_pull(5, gpiosim::Level::Low).is_ok());
        assert_eq!(s.get_pull(5).unwrap(), gpiosim::Level::Low);
        assert_eq!(req.value(5).unwrap(), line::Value::Inactive);
    }

    #[test]
    fn toggle() {
        let s = Simpleton::new(8);

        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(4)
            .as_input()
            .request();
        assert!(req.is_ok());
        let req = req.unwrap();

        assert_eq!(s.get_pull(4).unwrap(), gpiosim::Level::Low);
        assert_eq!(req.value(4).unwrap(), line::Value::Inactive);

        assert!(s.toggle(4).is_ok());
        assert_eq!(s.get_pull(4).unwrap(), gpiosim::Level::High);
        assert_eq!(req.value(4).unwrap(), line::Value::Active);

        assert!(s.toggle(4).is_ok());
        assert_eq!(s.get_pull(4).unwrap(), gpiosim::Level::Low);
        assert_eq!(req.value(4).unwrap(), line::Value::Inactive);
    }

    #[test]
    fn get_level() {
        let s = Simpleton::new(8);

        let req = Request::builder()
            .on_chip(s.dev_path())
            .with_line(3)
            .as_output(line::Value::Inactive)
            .request();
        assert!(req.is_ok());
        let req = req.unwrap();

        // chip pull checked to ensure not altered
        assert_eq!(s.get_pull(3).unwrap(), gpiosim::Level::Low);
        assert_eq!(req.value(3).unwrap(), line::Value::Inactive);

        assert!(req.set_value(3, line::Value::Active).is_ok());
        assert_eq!(s.get_pull(3).unwrap(), gpiosim::Level::Low);
        assert_eq!(s.get_level(3).unwrap(), gpiosim::Level::High);

        assert!(req.set_value(3, line::Value::Inactive).is_ok());
        assert_eq!(s.get_pull(3).unwrap(), gpiosim::Level::Low);
        assert_eq!(s.get_level(3).unwrap(), gpiosim::Level::Low);
    }
}
