#![no_std]

mod internal;

extern crate byteorder;
extern crate embedded_hal;

use embedded_hal::blocking::i2c;
pub use self::internal::MeasurementResolution;
use self::internal::{Humidity, SerialNumber, Temperature, UserHeaterRegister};

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    I2c(E),
    ChecksumFailure,
    NoPreviousHumidityMeasurement,
    InvalidHeaterLevel,
}

pub struct Si7021<I2C> {
    i2c: I2C,
}

pub type HeaterPower = u8;

const MEASURE_HUMIDITY_HOLD: &[u8] = &[0xe5];
const MEASURE_TEMPERATURE_HOLD: &[u8] = &[0xe3];
const READ_TEMPERATURE_FROM_HUMIDITY_MEASUREMENT: &[u8] = &[0xe0];
const RESET: &[u8] = &[0xfe];
const WRITE_USER_REGISTER1: &[u8] = &[0xe6];
const READ_USER_REGISTER1: &[u8] = &[0xe7];
const WRITE_HEATER_REGISTER: &[u8] = &[0x51];
const READ_HEATER_REGISTER: &[u8] = &[0x11];
const READ_ELECTRONIC_ID1: &[u8] = &[0xfa, 0x0f];
const READ_ELECTRONIC_ID2: &[u8] = &[0xfc, 0xc9];
const READ_FIRMWARE_REVISION: &[u8] = &[0x84, 0xb8];

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

    pub fn measurement_resolution(&mut self) -> Result<MeasurementResolution, Error<E>> {
        let mut user_heater_register: UserHeaterRegister<E> = UserHeaterRegister::new();
        self.write_read(READ_USER_REGISTER1, user_heater_register.buf_user())?;
        Ok(user_heater_register.measurement_resolution())
    }

    pub fn set_measurement_resolution(
        &mut self,
        measurement_resolution: MeasurementResolution,
    ) -> Result<(), Error<E>> {
        let mut user_heater_register: UserHeaterRegister<E> = UserHeaterRegister::new();
        self.write_read(READ_USER_REGISTER1, user_heater_register.buf_user())?;
        user_heater_register.set_measurement_resolution(measurement_resolution);
        self.i2c
            .write(
                0x40,
                &[WRITE_USER_REGISTER1[0], user_heater_register.buf_user()[0]],
            ).map_err(Error::I2c)?;
        Ok(())
    }

    pub fn heater(&mut self) -> Result<Option<HeaterPower>, Error<E>> {
        let mut user_heater_register: UserHeaterRegister<E> = UserHeaterRegister::new();
        self.write_read(READ_USER_REGISTER1, user_heater_register.buf_user())?;
        Ok(if user_heater_register.heater_on() {
            self.write_read(READ_HEATER_REGISTER, user_heater_register.buf_heater())?;
            Some(user_heater_register.heater_level())
        } else {
            None
        })
    }

    pub fn set_heater(&mut self, heater_power: Option<HeaterPower>) -> Result<(), Error<E>> {
        let mut user_heater_register: UserHeaterRegister<E> = UserHeaterRegister::new();
        self.write_read(READ_USER_REGISTER1, user_heater_register.buf_user())?;
        self.write_read(READ_HEATER_REGISTER, user_heater_register.buf_heater())?;
        match heater_power {
            Some(v) => {
                user_heater_register.set_heater_level(v)?;
                user_heater_register.set_heater_state(true)
            }
            None => user_heater_register.set_heater_state(false),
        }
        self.i2c
            .write(
                0x40,
                &[WRITE_USER_REGISTER1[0], user_heater_register.buf_user()[0]],
            ).map_err(Error::I2c)?;
        self.i2c
            .write(
                0x40,
                &[
                    WRITE_HEATER_REGISTER[0],
                    user_heater_register.buf_heater()[0],
                ],
            ).map_err(Error::I2c)?;
        Ok(())
    }
}
