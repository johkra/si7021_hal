use super::Error;
use byteorder::{BigEndian, ByteOrder};
use core::marker::PhantomData;

#[derive(Debug, PartialEq)]
pub enum MeasurementResolution {
    Rh12Temp14 = 0x00,
    Rh8Temp12 = 0x01,
    Rh10Temp10 = 0x80,
    Rh11Temp11 = 0x81,
}

#[derive(Default)]
pub struct Crc8 {
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

pub struct SerialNumber<E> {
    buffer: [u8; 14],
    _marker: PhantomData<E>,
}

impl<E> SerialNumber<E> {
    pub fn new() -> Self {
        SerialNumber {
            buffer: [0; 14],
            _marker: PhantomData,
        }
    }
    pub fn buf_id1(&mut self) -> &mut [u8] {
        &mut self.buffer[0..8]
    }
    pub fn buf_id2(&mut self) -> &mut [u8] {
        &mut self.buffer[8..14]
    }
    pub fn serial_number(&self) -> Result<u64, Error<E>> {
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

pub struct Temperature<E> {
    buffer: [u8; 3],
    _marker: PhantomData<E>,
}

impl<E> Temperature<E> {
    pub fn new() -> Self {
        Temperature {
            buffer: [0; 3],
            _marker: PhantomData,
        }
    }
    pub fn buf(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
    pub fn buf_nocrc(&mut self) -> &mut [u8] {
        &mut self.buffer[0..2]
    }
    pub fn temperature(&self) -> Result<i32, Error<E>> {
        let mut crc = Crc8::default();
        if crc.update(&self.buffer[0..2]) != self.buffer[2] {
            return Err(Error::ChecksumFailure);
        }
        self.temperature_nocrc()
    }
    pub fn temperature_nocrc(&self) -> Result<i32, Error<E>> {
        if self.buffer[0..2] == [0x00, 0x00] {
            return Err(Error::NoPreviousHumidityMeasurement);
        }
        Ok(((17572 * i32::from(BigEndian::read_u16(&self.buffer))) / 65536) - 4685)
    }
}

pub struct Humidity<E> {
    buffer: [u8; 3],
    _marker: PhantomData<E>,
}

impl<E> Humidity<E> {
    pub fn new() -> Self {
        Humidity {
            buffer: [0; 3],
            _marker: PhantomData,
        }
    }
    pub fn buf(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
    pub fn humidity(&self) -> Result<i32, Error<E>> {
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

pub struct UserHeaterRegister<E> {
    register: [u8; 2],
    _marker: PhantomData<E>,
}

const USER_REGISTER1: usize = 0;
const HEATER_REGISTER: usize = 1;
impl<E> UserHeaterRegister<E> {
    pub fn new() -> Self {
        UserHeaterRegister {
            register: [0; 2],
            _marker: PhantomData,
        }
    }
    pub fn buf_user(&mut self) -> &mut [u8] {
        &mut self.register[USER_REGISTER1..=USER_REGISTER1]
    }
    pub fn buf_heater(&mut self) -> &mut [u8] {
        &mut self.register[HEATER_REGISTER..=HEATER_REGISTER]
    }
    pub fn measurement_resolution(&self) -> MeasurementResolution {
        match self.register[USER_REGISTER1] & 0x81 {
            0x00 => MeasurementResolution::Rh12Temp14,
            0x01 => MeasurementResolution::Rh8Temp12,
            0x80 => MeasurementResolution::Rh10Temp10,
            // Use wildcard for 0x81 case, the compiler doesn't know all values have been covered
            _ => MeasurementResolution::Rh11Temp11,
        }
    }
    pub fn set_measurement_resolution(&mut self, measurement_resolution: MeasurementResolution) {
        self.register[USER_REGISTER1] =
            (self.register[USER_REGISTER1] & 0x7e) | measurement_resolution as u8
    }
    pub fn heater_on(&self) -> bool {
        self.register[USER_REGISTER1] & 0x04 > 0
    }
    pub fn set_heater_state(&mut self, on: bool) {
        self.register[USER_REGISTER1] =
            (self.register[USER_REGISTER1] & 0xfb) | (0x04 * u8::from(on))
    }
    pub fn heater_level(&self) -> u8 {
        self.register[HEATER_REGISTER] & 0x0f
    }
    pub fn set_heater_level(&mut self, heater_level: u8) -> Result<(), Error<E>> {
        if heater_level > 0x0f {
            return Err(Error::InvalidHeaterLevel);
        }
        self.register[HEATER_REGISTER] = (self.register[HEATER_REGISTER] & 0xf0) | heater_level;
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
