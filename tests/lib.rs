extern crate embedded_hal;
extern crate si7021_hal;

use embedded_hal::blocking::i2c::Write;
use embedded_hal::blocking::i2c::WriteRead;
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
enum Error {
    NoResponse,
    UnknownCommand,
}

// TODO: Move to embedded-hal-mock crate?
#[derive(Debug)]
struct MockI2c<'a> {
    request_response_map: HashMap<&'a [u8], &'a [u8]>,
}

impl<'a> WriteRead for MockI2c<'a> {
    type Error = Error;
    fn write_read(
        &mut self,
        _address: u8,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> Result<(), Self::Error> {
        match self.request_response_map.get(bytes) {
            Some(buf) => buffer.copy_from_slice(buf),
            None => return Err(Error::NoResponse),
        }
        Ok(())
    }
}

impl<'a> Write for MockI2c<'a> {
    type Error = Error;
    fn write(&mut self, _address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        if !self.request_response_map.contains_key(bytes) {
            return Err(Error::UnknownCommand);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use si7021_hal::MeasurementResolution;
    use si7021_hal::Si7021;
    use std::collections::HashMap;
    use super::Error;
    use super::MockI2c;

    // Type coercion from &[{integer}; N] to &[u8] failed when using the maplit macros.
    // Revisit usage of the crate after transitive coercions
    // (https://github.com/rust-lang/rust/issues/18602) are implemented?
    fn make_map<'a>(kvpairs: &[(&'a [u8], &'a [u8])]) -> HashMap<&'a [u8], &'a [u8]> {
        let mut map: HashMap<&[u8], &[u8]> = HashMap::new();
        for (k, v) in kvpairs {
            map.insert(k, v);
        }
        map
    }

    #[test]
    fn get_temperature() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xe3], &[0x66, 0x4c, 0x4f])]),
        });
        let temperature = si7021.temperature();
        assert!(temperature.is_ok());
        assert_eq!(temperature.unwrap(), 2336);
    }

    #[test]
    fn get_temperature_crc_failure() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xe3], &[0x66, 0x4c, 0xff])]),
        });
        let temperature = si7021.temperature();
        assert!(temperature.is_err());
        assert_eq!(temperature.unwrap_err(), si7021_hal::Error::ChecksumFailure);
    }

    #[test]
    fn get_humdity_and_temperature() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[
                (&[0xe5], &[0xa1, 0xa6, 0x51]),
                (&[0xe0], &[0x66, 0x44]),
            ]),
        });
        let humidity = si7021.humidity();
        assert!(humidity.is_ok());
        assert_eq!(humidity.unwrap(), 7292);

        let temperature = si7021.temperature_rh_measurement();
        assert!(temperature.is_ok());
        assert_eq!(temperature.unwrap(), 2334);
    }

    #[test]
    fn get_humdity_crc_failure() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xe5], &[0xa1, 0xa6, 0xff])]),
        });
        let humidity = si7021.humidity();
        assert!(humidity.is_err());
        assert_eq!(humidity.unwrap_err(), si7021_hal::Error::ChecksumFailure);
    }

    #[test]
    fn get_humidity_temperature_without_humidity_measurement() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xe0], &[0x00, 0x00])]),
        });
        let temperature = si7021.temperature_rh_measurement();
        assert!(temperature.is_err());
        assert_eq!(
            temperature.unwrap_err(),
            si7021_hal::Error::NoPreviousHumidityMeasurement
        );
    }

    #[test]
    fn get_serial_number() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[
                (
                    &[0xfa, 0x0f],
                    &[0x84, 0xbe, 0x2c, 0x5b, 0xf9, 0x9e, 0xb1, 0xa8],
                ),
                (&[0xfc, 0xc9], &[0x15, 0xff, 0xb5, 0xff, 0xff, 0xcb]),
            ]),
        });
        let serial_number = si7021.serial_number();
        assert!(serial_number.is_ok());
        assert_eq!(serial_number.unwrap(), 0x842cf9b115ffffff);
    }

    #[test]
    fn get_serial_number_crc_failure_id1() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[
                (
                    &[0xfa, 0x0f],
                    &[0x84, 0xbe, 0x2c, 0x5b, 0xf9, 0x9e, 0xb1, 0xff],
                ),
                (&[0xfc, 0xc9], &[0x15, 0xff, 0xb5, 0xff, 0xff, 0xcb]),
            ]),
        });
        let serial_number = si7021.serial_number();
        assert!(serial_number.is_err());
        assert_eq!(
            serial_number.unwrap_err(),
            si7021_hal::Error::ChecksumFailure
        );
    }

    #[test]
    fn get_serial_number_crc_failure_id2() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[
                (
                    &[0xfa, 0x0f],
                    &[0x84, 0xbe, 0x2c, 0x5b, 0xf9, 0x9e, 0xb1, 0xa8],
                ),
                (&[0xfc, 0xc9], &[0x15, 0xff, 0xb5, 0xff, 0xff, 0xff]),
            ]),
        });
        let serial_number = si7021.serial_number();
        assert!(serial_number.is_err());
        assert_eq!(
            serial_number.unwrap_err(),
            si7021_hal::Error::ChecksumFailure
        );
    }

    #[test]
    fn get_firmware_revision() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0x84, 0xb8], &[0x20])]),
        });
        let firmware_revision = si7021.firmware_revision();
        assert!(firmware_revision.is_ok());
        assert_eq!(firmware_revision.unwrap(), 0x20);
    }

    #[test]
    fn get_firmware_revision_i2c_error() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[]),
        });
        let firmware_revision = si7021.firmware_revision();
        assert!(firmware_revision.is_err());
        assert_eq!(
            firmware_revision.unwrap_err(),
            si7021_hal::Error::I2c(Error::NoResponse)
        );
    }

    #[test]
    fn reset() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xfe], &[])]),
        });
        let firmware_revision = si7021.reset();
        assert!(firmware_revision.is_ok());
    }

    #[test]
    fn set_measurement_resolution() {
        // Fill reserved bits with 1 and ensure they're written back
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xe7], &[0xff]), (&[0xe6, 0x7e], &[])]),
        });
        let measurement_resolution =
            si7021.set_measurement_resolution(MeasurementResolution::Rh12Temp14);
        assert!(measurement_resolution.is_ok());
    }

    #[test]
    fn get_measurement_resolution() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xe7], &[0x01])]),
        });
        let measurement_resolution = si7021.measurement_resolution();
        assert!(measurement_resolution.is_ok());
        assert_eq!(
            measurement_resolution.unwrap(),
            MeasurementResolution::Rh8Temp12
        );
    }

    #[test]
    fn get_heater_off() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xe7], &[0x00]), (&[0x11], &[0x00])]),
        });
        let heater = si7021.heater();
        assert!(heater.is_ok());
        assert_eq!(heater.unwrap(), None);
    }

    #[test]
    fn get_heater_on() {
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xe7], &[0x04]), (&[0x11], &[0x0a])]),
        });
        let heater = si7021.heater();
        assert!(heater.is_ok());
        assert_eq!(heater.unwrap(), Some(0x0a));
    }

    #[test]
    fn set_heater_off() {
        // Fill reserved bits with 1 and ensure they're written back
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[
                (&[0xe7], &[0xff]),
                (&[0x11], &[0xff]),
                (&[0xe6, 0xfb], &[]),
                (&[0x51, 0xff], &[]),
            ]),
        });
        let heater = si7021.set_heater(None);
        assert!(heater.is_ok());
    }

    #[test]
    fn set_heater_on() {
        // Fill reserved bits with 1 and ensure they're written back
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[
                (&[0xe7], &[0xfb]),
                (&[0x11], &[0xf0]),
                (&[0xe6, 0xff], &[]),
                (&[0x51, 0xfa], &[]),
            ]),
        });
        let heater = si7021.set_heater(Some(0x0a));
        assert!(heater.is_ok());
    }

    #[test]
    fn set_heater_invalid_power() {
        // Fill reserved bits with 1 and ensure they're written back
        let mut si7021 = Si7021::new(MockI2c {
            request_response_map: make_map(&[(&[0xe7], &[0xfb]), (&[0x11], &[0xf0])]),
        });
        let heater = si7021.set_heater(Some(0xf0));
        assert!(heater.is_err());
        assert_eq!(heater.unwrap_err(), si7021_hal::Error::InvalidHeaterLevel);
    }
}
