extern crate embedded_hal;
extern crate si7021_hal;

use embedded_hal::blocking::i2c::WriteRead;
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
enum Error {
    NoResponse,
}

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

#[cfg(test)]
mod tests {
    use si7021_hal::Si7021;
    use std::collections::HashMap;
    use Error;
    use MockI2c;

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
        assert_eq!(temperature.unwrap_err(), si7021_hal::Error::NoPreviousHumidityMeasurement);
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
}
