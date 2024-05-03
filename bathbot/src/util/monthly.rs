use std::ops::{Add, Range, Sub};

use plotters::{
    coord::ranged1d::{DefaultFormatting, KeyPointHint, NoDefaultFormatting, ValueFormatter},
    prelude::{DiscreteRanged, Ranged},
};
use time::{Date, Duration, Month, OffsetDateTime, Time};

pub trait IsDate: Copy {
    fn year(self) -> i32;
    fn month(self) -> Month;
    fn day(self) -> u8;
}

impl IsDate for Date {
    fn year(self) -> i32 {
        Self::year(self)
    }

    fn month(self) -> Month {
        Self::month(self)
    }

    fn day(self) -> u8 {
        Self::day(self)
    }
}

impl IsDate for OffsetDateTime {
    fn year(self) -> i32 {
        Self::year(self)
    }

    fn month(self) -> Month {
        Self::month(self)
    }

    fn day(self) -> u8 {
        Self::day(self)
    }
}

/// The trait that describe some time value. This is the uniformed abstraction
/// that works for both Date, DateTime and Duration, etc.
pub trait TimeValue: Copy + Eq {
    type DateType: IsDate + PartialOrd;

    /// Returns the date that is no later than the time
    fn date_floor(&self) -> Self::DateType;
    /// Returns the date that is no earlier than the time
    fn date_ceil(&self) -> Self::DateType;
    /// Returns the maximum value that is earlier than the given date
    fn earliest_after_date(date: Self::DateType) -> Self;
    /// Returns the duration between two time value
    fn subtract(&self, other: &Self) -> Duration;
    /// Instantiate a date type for current time value;
    fn ymd(year: i32, month: Month, day: u8) -> Self::DateType;

    /// Map the coord spec
    fn map_coord(value: &Self, begin: &Self, end: &Self, (min, max): (i32, i32)) -> i32 {
        let total_span = end.subtract(begin);
        let value_span = value.subtract(begin);

        let total_ns = total_span.whole_nanoseconds();
        let value_ns = value_span.whole_nanoseconds();

        ((max - min) as f64 * value_ns as f64 / total_ns as f64) as i32 + min
    }
}

impl TimeValue for Date {
    type DateType = Self;

    fn date_floor(&self) -> Date {
        *self
    }

    fn date_ceil(&self) -> Date {
        *self
    }

    fn earliest_after_date(date: Date) -> Self {
        date
    }

    fn subtract(&self, other: &Date) -> Duration {
        *self - *other
    }

    fn ymd(year: i32, month: Month, day: u8) -> Self::DateType {
        Date::from_calendar_date(year, month, day).unwrap()
    }
}

impl TimeValue for OffsetDateTime {
    type DateType = Date;

    fn date_floor(&self) -> Self::DateType {
        self.date()
    }

    fn date_ceil(&self) -> Self::DateType {
        if self.time() == Time::MIDNIGHT {
            self.date().next_day().unwrap()
        } else {
            self.date()
        }
    }

    fn earliest_after_date(date: Self::DateType) -> Self {
        date.with_hms(0, 0, 0).unwrap().assume_utc()
    }

    fn subtract(&self, other: &Self) -> Duration {
        *self - *other
    }

    fn ymd(year: i32, month: Month, day: u8) -> Self::DateType {
        Date::from_calendar_date(year, month, day).unwrap()
    }
}

/// The ranged coordinate for date
#[derive(Copy, Clone)]
pub struct RangedDate<T>(T, T);

impl<T> From<Range<T>> for RangedDate<T> {
    #[inline]
    fn from(range: Range<T>) -> Self {
        Self(range.start, range.end)
    }
}

impl<T> Ranged for RangedDate<T>
where
    T: TimeValue + Add<Duration, Output = T> + Sub<T, Output = Duration>,
{
    type FormatOption = DefaultFormatting;
    type ValueType = T;

    fn range(&self) -> Range<T> {
        self.0..self.1
    }

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        TimeValue::map_coord(value, &self.0, &self.1, limit)
    }

    fn key_points<HintType: KeyPointHint>(&self, hint: HintType) -> Vec<Self::ValueType> {
        let max_points = hint.max_num_points();
        let mut ret = vec![];

        let total_days = (self.1 - self.0).whole_days();
        let total_weeks = (self.1 - self.0).whole_weeks();

        if total_days > 0 && total_days as usize <= max_points {
            for day_idx in 0..=total_days {
                ret.push(self.0 + Duration::days(day_idx));
            }

            return ret;
        }

        if total_weeks > 0 && total_weeks as usize <= max_points {
            for day_idx in 0..=total_weeks {
                ret.push(self.0 + Duration::weeks(day_idx));
            }

            return ret;
        }

        // When all data is in the same week, just plot properly.
        if total_weeks == 0 {
            ret.push(self.0);

            return ret;
        }

        let week_per_point = ((total_weeks as f64) / (max_points as f64)).ceil() as usize;

        for idx in 0..=(total_weeks as usize / week_per_point) {
            ret.push(self.0 + Duration::weeks((idx * week_per_point) as i64));
        }

        ret
    }
}

impl<T> DiscreteRanged for RangedDate<T>
where
    T: TimeValue + Sub<T, Output = Duration> + Add<Duration, Output = T>,
{
    fn size(&self) -> usize {
        ((self.1 - self.0).whole_days().max(-1) + 1) as usize
    }

    fn index_of(&self, value: &T) -> Option<usize> {
        let ret = (*value - self.0).whole_days();
        if ret < 0 {
            return None;
        }
        Some(ret as usize)
    }

    fn from_index(&self, index: usize) -> Option<T> {
        Some(self.0 + Duration::days(index as i64))
    }
}

/// Indicates the coord has a monthly resolution
///
/// Note: since month doesn't have a constant duration.
/// We can't use a simple granularity to describe it. Thus we have
/// this axis decorator to make it yield monthly key-points.
#[derive(Clone)]
pub struct Monthly<T>(pub Range<T>);

impl<T: IsDate> ValueFormatter<T> for Monthly<T> {
    fn format(value: &T) -> String {
        format!("{}-{}", value.year(), value.month() as u8)
    }
}

impl<T: TimeValue> Monthly<T> {
    fn bold_key_points<H: KeyPointHint>(&self, hint: &H) -> Vec<T> {
        let max_points = hint.max_num_points();
        let start_date = self.0.start.date_ceil();
        let end_date = self.0.end.date_floor();

        let mut start_year = start_date.year();
        let mut start_month = start_date.month();
        let start_day = start_date.day();

        let end_year = end_date.year();
        let end_month = end_date.month();

        if start_day != 1 {
            start_month = start_month.next();
            start_year += (start_month == Month::January) as i32;
        }

        let total_month = (end_year - start_year) * 12 + end_month as i32 - start_month as i32;

        fn generate_key_points<T: TimeValue>(
            mut start_year: i32,
            mut start_month: u8,
            end_year: i32,
            end_month: u8,
            step: u8,
        ) -> Vec<T> {
            let mut ret = vec![];
            while end_year > start_year || (end_year == start_year && end_month >= start_month) {
                ret.push(T::earliest_after_date(T::ymd(
                    start_year,
                    Month::try_from(start_month).unwrap(),
                    1,
                )));

                start_month += step;

                if start_month >= 13 {
                    start_year += start_month as i32 / 12;
                    start_month %= 12;
                }
            }

            ret
        }

        if total_month as usize <= max_points {
            // Monthly
            generate_key_points(start_year, start_month as u8, end_year, end_month as u8, 1)
        } else if total_month as usize <= max_points * 3 {
            // Quarterly
            generate_key_points(start_year, start_month as u8, end_year, end_month as u8, 3)
        } else if total_month as usize <= max_points * 6 {
            // Biyearly
            generate_key_points(start_year, start_month as u8, end_year, end_month as u8, 6)
        } else {
            // Otherwise we could generate the yearly keypoints
            generate_yearly_keypoints(
                max_points,
                start_year,
                start_month as u8,
                end_year,
                end_month as u8,
            )
        }
    }
}

impl<T> Ranged for Monthly<T>
where
    T: TimeValue,
    RangedDate<T>: Ranged<ValueType = T>,
{
    type FormatOption = NoDefaultFormatting;
    type ValueType = T;

    fn range(&self) -> Range<T> {
        self.0.start..self.0.end
    }

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        T::map_coord(value, &self.0.start, &self.0.end, limit)
    }

    fn key_points<HintType: KeyPointHint>(&self, hint: HintType) -> Vec<Self::ValueType> {
        if hint.weight().allow_light_points() && self.size() <= hint.bold_points() * 2 {
            let coord = RangedDate::<T>::from(self.0.clone());

            return coord.key_points(hint.max_num_points());
        }

        self.bold_key_points(&hint)
    }
}

impl<T> DiscreteRanged for Monthly<T>
where
    T: TimeValue,
    RangedDate<T>: Ranged<ValueType = T>,
{
    fn size(&self) -> usize {
        let (start_year, start_month) = {
            let ceil = self.0.start.date_ceil();
            (ceil.year(), ceil.month())
        };

        let (end_year, end_month) = {
            let floor = self.0.end.date_floor();
            (floor.year(), floor.month())
        };

        ((end_year - start_year).max(0) * 12
            + (1 - start_month as i32)
            + (end_month as i32 - 1)
            + 1)
        .max(0) as usize
    }

    fn index_of(&self, value: &T) -> Option<usize> {
        let this_year = value.date_floor().year();
        let this_month = value.date_floor().month();

        let start_year = self.0.start.date_ceil().year();
        let start_month = self.0.start.date_ceil().month();

        let ret = (this_year - start_year).max(0) * 12
            + (1 - start_month as i32)
            + (this_month as i32 - 1);

        if ret >= 0 {
            return Some(ret as usize);
        }

        None
    }

    fn from_index(&self, index: usize) -> Option<T> {
        if index == 0 {
            return Some(T::earliest_after_date(self.0.start.date_ceil()));
        }

        let index_from_start_year = index + (self.0.start.date_ceil().month() as u8 - 1) as usize;
        let year = self.0.start.date_ceil().year() + index_from_start_year as i32 / 12;
        let month = (index_from_start_year % 12) as u8 + 1;

        Some(T::earliest_after_date(T::ymd(
            year,
            Month::try_from(month).unwrap(),
            1,
        )))
    }
}

fn generate_yearly_keypoints<T: TimeValue>(
    max_points: usize,
    mut start_year: i32,
    start_month: u8,
    mut end_year: i32,
    end_month: u8,
) -> Vec<T> {
    if start_month > end_month {
        end_year -= 1;
    }

    let mut exp10 = 1;

    while (end_year - start_year + 1) as usize / (exp10 * 10) > max_points {
        exp10 *= 10;
    }

    let mut freq = exp10;

    for try_freq in &[1, 2, 5, 10] {
        freq = *try_freq * exp10;

        if (end_year - start_year + 1) as usize / (exp10 * *try_freq) <= max_points {
            break;
        }
    }

    let mut ret = vec![];

    while start_year <= end_year {
        ret.push(T::earliest_after_date(T::ymd(
            start_year,
            Month::try_from(start_month).unwrap(),
            1,
        )));

        start_year += freq as i32;
    }

    ret
}
