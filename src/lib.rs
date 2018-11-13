#![no_std]

#[macro_use]
extern crate bitfield;
extern crate byteorder;
extern crate embedded_hal;

use byteorder::{BigEndian, ByteOrder};
use core::marker::PhantomData;
use embedded_hal::blocking::i2c;

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    I2c(E),
    ChecksumFailure,
    NoPreviousHumidityMeasurement,
}

pub struct Si7021<I2C> {
    i2c: I2C,
}

const MEASURE_HUMIDITY_HOLD: &[u8] = &[0xe5];
const MEASURE_TEMPERATURE_HOLD: &[u8] = &[0xe3];
const READ_TEMPERATURE_FROM_HUMIDITY_MEASUREMENT: &[u8] = &[0xe0];
const RESET: &[u8] = &[0xfe];
const READ_USER_REGISTER1: &[u8] = &[0xe7];
const READ_HEATER_REGISTER: &[u8] = &[0x11];
const READ_ELECTRONIC_ID1: &[u8] = &[0xfa, 0x0f];
const READ_ELECTRONIC_ID2: &[u8] = &[0xfc, 0xc9];
const READ_FIRMWARE_REVISION: &[u8] = &[0x84, 0xb8];

#[derive(Default)]
struct Crc8 {
    crc: u8,
}

impl Crc8 {
    pub fn update(&mut self, input: &[u8]) -> u8 {
        for b in input {
            self.crc ^= *b;
            for _ in 0..8 {
                if self.crc & 0x80 == 0 {
                    self.crc <<= 1;
                } else {
                    self.crc = (self.crc << 1) ^ 0x31;
                }
            }
        }
        self.crc
    }
}

struct SerialNumber<E> {
    buffer: [u8; 14],
    _marker: PhantomData<E>,
}

impl<E> SerialNumber<E> {
    fn new() -> Self {
        SerialNumber {
            buffer: [0; 14],
            _marker: PhantomData,
        }
    }
    fn buf_id1(&mut self) -> &mut [u8] {
        &mut self.buffer[0..8]
    }
    fn buf_id2(&mut self) -> &mut [u8] {
        &mut self.buffer[8..14]
    }
    fn serial_number(&self) -> Result<u64, Error<E>> {
        let mut crc = Crc8::default();
        let (sna_3, sna_2, sna_1, sna_0, crc_a) = (
            self.buffer[0],
            self.buffer[2],
            self.buffer[4],
            self.buffer[6],
            self.buffer[7],
        );
        if crc.update(&[sna_3, sna_2, sna_1, sna_0]) != crc_a {
            return Err(Error::ChecksumFailure);
        }
        let mut crc = Crc8::default();
        let (snb_3, snb_2, snb_1, snb_0, crc_b) = (
            self.buffer[8],
            self.buffer[9],
            self.buffer[11],
            self.buffer[12],
            self.buffer[13],
        );
        if crc.update(&[snb_3, snb_2, snb_1, snb_0]) != crc_b {
            return Err(Error::ChecksumFailure);
        }
        Ok(u64::from(sna_3) << 56
            | u64::from(sna_2) << 48
            | u64::from(sna_1) << 40
            | u64::from(sna_0) << 32
            | u64::from(snb_3) << 24
            | u64::from(snb_2) << 16
            | u64::from(snb_1) << 8
            | u64::from(snb_0))
    }
}

struct Temperature<E> {
    buffer: [u8; 3],
    _marker: PhantomData<E>,
}

impl<E> Temperature<E> {
    fn new() -> Self {
        Temperature {
            buffer: [0; 3],
            _marker: PhantomData,
        }
    }
    fn buf(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
    fn buf_nocrc(&mut self) -> &mut [u8] {
        &mut self.buffer[0..2]
    }
    fn temperature(&self) -> Result<i32, Error<E>> {
        let mut crc = Crc8::default();
        if crc.update(&self.buffer[0..2]) != self.buffer[2] {
            return Err(Error::ChecksumFailure);
        }
        self.temperature_nocrc()
    }
    fn temperature_nocrc(&self) -> Result<i32, Error<E>> {
        if self.buffer[0..2] == [0x00, 0x00] {
            return Err(Error::NoPreviousHumidityMeasurement);
        }
        Ok(((17572 * i32::from(BigEndian::read_u16(&self.buffer))) / 65536) - 4685)
    }
}

struct Humidity<E> {
    buffer: [u8; 3],
    _marker: PhantomData<E>,
}

impl<E> Humidity<E> {
    fn new() -> Self {
        Humidity {
            buffer: [0; 3],
            _marker: PhantomData,
        }
    }
    fn buf(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
    fn humidity(&self) -> Result<i32, Error<E>> {
        let mut crc = Crc8::default();
        if crc.update(&self.buffer[0..2]) != self.buffer[2] {
            return Err(Error::ChecksumFailure);
        }
        let val = ((12500 * i32::from(BigEndian::read_u16(&self.buffer))) / 65536) - 600;
        Ok(match val {
            rh if rh > 10000 => 10000,
            rh if rh < 0 => 0,
            rh => rh,
        })
    }
}

bitfield!{
    struct UserRegister1(u8);
    impl Debug;
    res1, set_res1: 7;
    vdds, _: 6;
    htre, set_htre: 2;
    res0, set_res0: 0;
}

enum MeasurementResolution {
    Rh12Temp14,
    Rh8Temp12,
    Rh10Temp10,
    Rh11Temp11,
}

bitfield!{
    struct HeaterRegister(u8);
    impl Debug;
    heater, set_heater: 3, 0;
}

impl<E, I2C> Si7021<I2C>
where
    I2C: i2c::WriteRead<Error = E> + i2c::Write<Error = E>,
{
    pub fn new(i2c: I2C) -> Self {
        Si7021 { i2c }
    }

    fn write_read(&mut self, command: &[u8], buffer: &mut [u8]) -> Result<(), Error<E>> {
        self.i2c
            .write_read(0x40, command, buffer)
            .map_err(Error::I2c)?;
        Ok(())
    }

    // Returns relative humidity in % scaled by 100, i.e. 23.15% returns 2315
    pub fn humidity(&mut self) -> Result<i32, Error<E>> {
        let mut humidity: Humidity<E> = Humidity::new();
        self.write_read(MEASURE_HUMIDITY_HOLD, humidity.buf())?;
        humidity.humidity()
    }

    // Returns temperature in °C scaled by 100, i.e. 23.15°C returns 2315
    // Temperature taken during last relative humidity measurement
    pub fn temperature_rh_measurement(&mut self) -> Result<i32, Error<E>> {
        let mut temperature: Temperature<E> = Temperature::new();
        self.write_read(
            READ_TEMPERATURE_FROM_HUMIDITY_MEASUREMENT,
            temperature.buf_nocrc(),
        )?;
        temperature.temperature_nocrc()
    }

    // Returns temperature in °C scaled by 100, i.e. 23.15°C returns 2315
    pub fn temperature(&mut self) -> Result<i32, Error<E>> {
        let mut temperature: Temperature<E> = Temperature::new();
        self.write_read(MEASURE_TEMPERATURE_HOLD, temperature.buf())?;
        temperature.temperature()
    }

    pub fn serial_number(&mut self) -> Result<u64, Error<E>> {
        let mut serial_number: SerialNumber<E> = SerialNumber::new();
        self.write_read(READ_ELECTRONIC_ID1, serial_number.buf_id1())?;
        self.write_read(READ_ELECTRONIC_ID2, serial_number.buf_id2())?;
        serial_number.serial_number()
    }

    pub fn firmware_revision(&mut self) -> Result<u8, Error<E>> {
        let mut buffer = [0u8; 1];
        self.write_read(READ_FIRMWARE_REVISION, &mut buffer)?;
        Ok(buffer[0])
    }

    pub fn reset(&mut self) -> Result<(), Error<E>> {
        self.i2c.write(0x40, RESET).map_err(Error::I2c)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Crc8;

    #[test]
    fn update_crc() {
        let input = &[0x84, 0x2c, 0xf9, 0xb1];
        let mut crc = Crc8::default();
        assert_eq!(crc.update(input), 0xa8);
    }

    #[test]
    fn update_crc3() {
        let input = &[0x15, 0xff, 0xff, 0xff];
        let mut crc = Crc8::default();
        assert_eq!(crc.update(input), 0xcb);
    }

    #[test]
    fn update_crc_chunks() {
        let input_expected = &[0x84u8, 0xbe, 0x2c, 0x5b, 0xf9, 0x9e, 0xb1, 0xa8];
        let mut crc = Crc8::default();
        for chunk in input_expected.chunks(2) {
            let (input, expected) = (chunk[0], chunk[1]);
            assert_eq!(crc.update(&[input]), expected);
        }
    }
}
