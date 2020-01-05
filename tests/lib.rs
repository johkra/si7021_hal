#[cfg(test)]
mod tests {
    use embedded_hal_mock::i2c::{Mock as I2cMock, Transaction as I2cTransaction};
    use embedded_hal_mock::MockError;
    use si7021_hal::MeasurementResolution;
    use si7021_hal::Si7021;
    use std::io::ErrorKind;

    #[test]
    fn get_temperature() {
        let mut si7021 = Si7021::new(I2cMock::new(&[I2cTransaction::write_read(
            0x40,
            vec![0xe3],
            vec![0x66, 0x4c, 0x4f],
        )]));

        let temperature = si7021.temperature();
        assert!(temperature.is_ok());
        assert_eq!(temperature.unwrap(), 2336);
    }

    #[test]
    fn get_temperature_crc_failure() {
        let mut si7021 = Si7021::new(I2cMock::new(&[I2cTransaction::write_read(
            0x40,
            vec![0xe3],
            vec![0x66, 0x4c, 0xff],
        )]));

        let temperature = si7021.temperature();
        assert!(temperature.is_err());
        assert_eq!(temperature.unwrap_err(), si7021_hal::Error::ChecksumFailure);
    }

    #[test]
    fn get_humdity_and_temperature() {
        let mut si7021 = Si7021::new(I2cMock::new(&[
            I2cTransaction::write_read(0x40, vec![0xe5], vec![0xa1, 0xa6, 0x51]),
            I2cTransaction::write_read(0x40, vec![0xe0], vec![0x66, 0x44]),
        ]));

        let humidity = si7021.humidity();
        assert!(humidity.is_ok());
        assert_eq!(humidity.unwrap(), 7292);

        let temperature = si7021.temperature_rh_measurement();
        assert!(temperature.is_ok());
        assert_eq!(temperature.unwrap(), 2334);
    }

    #[test]
    fn get_humdity_crc_failure() {
        let mut si7021 = Si7021::new(I2cMock::new(&[I2cTransaction::write_read(
            0x40,
            vec![0xe5],
            vec![0xa1, 0xa6, 0xff],
        )]));

        let humidity = si7021.humidity();
        assert!(humidity.is_err());
        assert_eq!(humidity.unwrap_err(), si7021_hal::Error::ChecksumFailure);
    }

    #[test]
    fn get_humidity_temperature_without_humidity_measurement() {
        let mut si7021 = Si7021::new(I2cMock::new(&[I2cTransaction::write_read(
            0x40,
            vec![0xe0],
            vec![0x00, 0x00],
        )]));

        let temperature = si7021.temperature_rh_measurement();
        assert!(temperature.is_err());
        assert_eq!(
            temperature.unwrap_err(),
            si7021_hal::Error::NoPreviousHumidityMeasurement
        );
    }

    #[test]
    fn get_serial_number() {
        let mut si7021 = Si7021::new(I2cMock::new(&[
            I2cTransaction::write_read(
                0x40,
                vec![0xfa, 0x0f],
                vec![0x84, 0xbe, 0x2c, 0x5b, 0xf9, 0x9e, 0xb1, 0xa8],
            ),
            I2cTransaction::write_read(
                0x40,
                vec![0xfc, 0xc9],
                vec![0x15, 0xff, 0xb5, 0xff, 0xff, 0xcb],
            ),
        ]));

        let serial_number = si7021.serial_number();
        assert!(serial_number.is_ok());
        assert_eq!(serial_number.unwrap(), 0x842cf9b115ffffff);
    }

    #[test]
    fn get_serial_number_crc_failure_id1() {
        let mut si7021 = Si7021::new(I2cMock::new(&[
            I2cTransaction::write_read(
                0x40,
                vec![0xfa, 0x0f],
                vec![0x84, 0xbe, 0x2c, 0x5b, 0xf9, 0x9e, 0xb1, 0xff],
            ),
            I2cTransaction::write_read(
                0x40,
                vec![0xfc, 0xc9],
                vec![0x15, 0xff, 0xb5, 0xff, 0xff, 0xcb],
            ),
        ]));

        let serial_number = si7021.serial_number();
        assert!(serial_number.is_err());
        assert_eq!(
            serial_number.unwrap_err(),
            si7021_hal::Error::ChecksumFailure
        );
    }

    #[test]
    fn get_serial_number_crc_failure_id2() {
        let mut si7021 = Si7021::new(I2cMock::new(&[
            I2cTransaction::write_read(
                0x40,
                vec![0xfa, 0x0f],
                vec![0x84, 0xbe, 0x2c, 0x5b, 0xf9, 0x9e, 0xb1, 0xa8],
            ),
            I2cTransaction::write_read(
                0x40,
                vec![0xfc, 0xc9],
                vec![0x15, 0xff, 0xb5, 0xff, 0xff, 0xff],
            ),
        ]));

        let serial_number = si7021.serial_number();
        assert!(serial_number.is_err());
        assert_eq!(
            serial_number.unwrap_err(),
            si7021_hal::Error::ChecksumFailure
        );
    }

    #[test]
    fn get_firmware_revision() {
        let mut si7021 = Si7021::new(I2cMock::new(&[I2cTransaction::write_read(
            0x40,
            vec![0x84, 0xb8],
            vec![0x20],
        )]));

        let firmware_revision = si7021.firmware_revision();
        assert!(firmware_revision.is_ok());
        assert_eq!(firmware_revision.unwrap(), 0x20);
    }

    #[test]
    fn get_firmware_revision_i2c_error() {
        let mut si7021 = Si7021::new(I2cMock::new(&[I2cTransaction::write_read(
            0x40,
            vec![0x84, 0xb8],
            vec![0x20],
        )
        .with_error(MockError::Io(ErrorKind::Other))]));

        let firmware_revision = si7021.firmware_revision();
        assert!(firmware_revision.is_err());
        assert_eq!(
            firmware_revision.unwrap_err(),
            si7021_hal::Error::I2c(MockError::Io(ErrorKind::Other))
        );
    }

    #[test]
    fn reset() {
        let mut si7021 = Si7021::new(I2cMock::new(&[I2cTransaction::write(0x40, vec![0xfe])]));

        let reset = si7021.reset();
        assert!(reset.is_ok());
    }

    #[test]
    fn set_measurement_resolution() {
        // Fill reserved bits with 1 and ensure they're written back
        let mut si7021 = Si7021::new(I2cMock::new(&[
            I2cTransaction::write_read(0x40, vec![0xe7], vec![0xff]),
            I2cTransaction::write(0x40, vec![0xe6, 0x7e]),
        ]));

        let measurement_resolution =
            si7021.set_measurement_resolution(MeasurementResolution::Rh12Temp14);
        assert!(measurement_resolution.is_ok());
    }

    #[test]
    fn get_measurement_resolution() {
        let mut si7021 = Si7021::new(I2cMock::new(&[I2cTransaction::write_read(
            0x40,
            vec![0xe7],
            vec![0x01],
        )]));

        let measurement_resolution = si7021.measurement_resolution();
        assert!(measurement_resolution.is_ok());
        assert_eq!(
            measurement_resolution.unwrap(),
            MeasurementResolution::Rh8Temp12
        );
    }

    #[test]
    fn get_heater_off() {
        let mut si7021 = Si7021::new(I2cMock::new(&[I2cTransaction::write_read(
            0x40,
            vec![0xe7],
            vec![0x11],
        )]));

        let heater = si7021.heater();
        assert!(heater.is_ok());
        assert_eq!(heater.unwrap(), None);
    }

    #[test]
    fn get_heater_on() {
        let mut si7021 = Si7021::new(I2cMock::new(&[
            I2cTransaction::write_read(0x40, vec![0xe7], vec![0x04]),
            I2cTransaction::write_read(0x40, vec![0x11], vec![0x0a]),
        ]));

        let heater = si7021.heater();
        assert!(heater.is_ok());
        assert_eq!(heater.unwrap(), Some(0x0a));
    }

    #[test]
    fn set_heater_off() {
        // Fill reserved bits with 1 and ensure they're written back
        let mut si7021 = Si7021::new(I2cMock::new(&[
            I2cTransaction::write_read(0x40, vec![0xe7], vec![0xff]),
            I2cTransaction::write_read(0x40, vec![0x11], vec![0xff]),
            I2cTransaction::write(0x40, vec![0xe6, 0xfb]),
            I2cTransaction::write(0x40, vec![0x51, 0xff]),
        ]));

        let heater = si7021.set_heater(None);
        assert!(heater.is_ok());
    }

    #[test]
    fn set_heater_on() {
        // Fill reserved bits with 1 and ensure they're written back
        let mut si7021 = Si7021::new(I2cMock::new(&[
            I2cTransaction::write_read(0x40, vec![0xe7], vec![0xfb]),
            I2cTransaction::write_read(0x40, vec![0x11], vec![0xf0]),
            I2cTransaction::write(0x40, vec![0xe6, 0xff]),
            I2cTransaction::write(0x40, vec![0x51, 0xfa]),
        ]));

        let heater = si7021.set_heater(Some(0x0a));
        assert!(heater.is_ok());
    }

    #[test]
    fn set_heater_invalid_power() {
        // Fill reserved bits with 1 and ensure they're written back
        let mut si7021 = Si7021::new(I2cMock::new(&[
            I2cTransaction::write_read(0x40, vec![0xe7], vec![0xfb]),
            I2cTransaction::write_read(0x40, vec![0x11], vec![0xf0]),
        ]));

        let heater = si7021.set_heater(Some(0xf0));
        assert!(heater.is_err());
        assert_eq!(heater.unwrap_err(), si7021_hal::Error::InvalidHeaterLevel);
    }
}
