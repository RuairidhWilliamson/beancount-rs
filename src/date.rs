use std::{convert::Infallible, num::ParseIntError};

#[derive(Debug)]
pub enum DateError<T> {
    InvalidYear,
    InvalidMonth,
    InvalidDay,
    Other(T),
}

impl<T: std::fmt::Display> std::fmt::Display for DateError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidYear => f.write_str("invalid year"),
            Self::InvalidMonth => f.write_str("invalid month"),
            Self::InvalidDay => f.write_str("invalid day"),
            Self::Other(err) => f.write_fmt(format_args!("date error: {err}")),
        }
    }
}

impl<T> From<T> for DateError<T> {
    fn from(value: T) -> Self {
        Self::Other(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: u16,
    // Must be 1..=12
    month: u8,
    // Must be 1..=31 and be in range for the current month and year
    day: u8,
}

impl Date {
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, DateError<Infallible>> {
        if day == 0 || day > Self::month_days(year, month).ok_or(DateError::InvalidMonth)? {
            return Err(DateError::InvalidDay);
        }
        Ok(Self { year, month, day })
    }

    pub fn parse_uk(s: &str) -> Result<Self, DateError<ParseIntError>> {
        let (day, rest) = s.split_once(' ').ok_or(DateError::InvalidMonth)?;
        let (month, year) = rest.split_once(' ').ok_or(DateError::InvalidYear)?;
        let day: u8 = day.parse()?;
        let month = match month {
            "Jan" | "January" => 1,
            "Feb" | "February" => 2,
            "Mar" | "March" => 3,
            "Apr" | "April" => 4,
            "May" => 5,
            "Jun" | "June" => 6,
            "Jul" | "July" => 7,
            "Aug" | "August" => 8,
            "Sep" | "September" => 9,
            "Oct" | "October" => 10,
            "Nov" | "November" => 11,
            "Dec" | "December" => 12,
            _ => return Err(DateError::InvalidMonth),
        };
        let year = if year.len() == 2 {
            let year_short: u8 = year.parse()?;
            u16::from(year_short) + 2000
        } else if year.len() == 4 {
            year.parse()?
        } else {
            return Err(DateError::InvalidYear);
        };
        let res = Self::new(year, month, day);
        match res {
            Ok(value) => Ok(value),
            Err(DateError::InvalidYear) => Err(DateError::InvalidYear),
            Err(DateError::InvalidMonth) => Err(DateError::InvalidMonth),
            Err(DateError::InvalidDay) => Err(DateError::InvalidDay),
        }
    }

    pub fn year(&self) -> u16 {
        self.year
    }

    pub fn month(&self) -> u8 {
        self.month
    }

    pub fn day(&self) -> u8 {
        self.day
    }

    fn leap_year(year: u16) -> bool {
        year.is_multiple_of(400) || year.is_multiple_of(4) && !year.is_multiple_of(100)
    }

    fn month_days(year: u16, month: u8) -> Option<u8> {
        match month {
            2 if Self::leap_year(year) => Some(29),
            2 => Some(28),
            4 | 6 | 9 | 11 => Some(30),
            1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
            _ => None,
        }
    }

    #[must_use]
    pub fn next_day(&self) -> Self {
        let mut day = self.day + 1;
        let mut month = self.month;
        let mut year = self.year;
        if day > Self::month_days(self.year, self.month).expect("self.month is a valid month") {
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
        Self { year, month, day }
    }
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        ))
    }
}

impl From<jiff::civil::Date> for Date {
    fn from(value: jiff::civil::Date) -> Self {
        Self {
            year: value.year().cast_unsigned(),
            month: value.month().cast_unsigned(),
            day: value.day().cast_unsigned(),
        }
    }
}
