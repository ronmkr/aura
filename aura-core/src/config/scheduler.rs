use crate::config::bandwidth::{BandwidthConfig, BandwidthSchedule};
use chrono::{DateTime, Datelike, Local, TimeZone, Timelike, Utc};

pub struct BandwidthScheduler;

impl BandwidthScheduler {
    pub fn effective_limits(
        config: &BandwidthConfig,
        now_utc: DateTime<Utc>,
    ) -> (u64, u64, Option<BandwidthSchedule>) {
        if config.schedule.is_empty() {
            return (
                config.global_download_limit,
                config.global_upload_limit,
                None,
            );
        }

        let mut matching_schedules = Vec::new();

        for entry in &config.schedule {
            if Self::matches_schedule(entry, now_utc) {
                matching_schedules.push(entry);
            }
        }

        if matching_schedules.is_empty() {
            return (
                config.global_download_limit,
                config.global_upload_limit,
                None,
            );
        }

        // Find the best match by specificity and then index (last-write-wins)
        let mut best_match: Option<(&BandwidthSchedule, usize)> = None;

        for (idx, entry) in matching_schedules.into_iter().enumerate() {
            match best_match {
                None => best_match = Some((entry, idx)),
                Some((best, _best_idx)) => {
                    let best_spec = Self::specificity(best);
                    let entry_spec = Self::specificity(entry);
                    if entry_spec >= best_spec {
                        best_match = Some((entry, idx));
                    }
                }
            }
        }

        if let Some((best, _)) = best_match {
            (best.download_limit, best.upload_limit, Some(best.clone()))
        } else {
            (
                config.global_download_limit,
                config.global_upload_limit,
                None,
            )
        }
    }

    fn specificity(entry: &BandwidthSchedule) -> usize {
        if entry.days.is_empty() {
            0
        } else {
            8 - entry.days.len().min(7)
        }
    }

    fn matches_schedule(entry: &BandwidthSchedule, now_utc: DateTime<Utc>) -> bool {
        let day_str: String;
        let hour: u32;
        let minute: u32;

        if let Some(ref tz_str) = entry.timezone {
            if let Ok(tz) = tz_str.parse::<chrono_tz::Tz>() {
                let local_time = now_utc.with_timezone(&tz);
                day_str = local_time.format("%a").to_string();
                hour = local_time.hour();
                minute = local_time.minute();
            } else {
                let local_time = Local::now();
                day_str = local_time.format("%a").to_string();
                hour = local_time.hour();
                minute = local_time.minute();
            }
        } else {
            // Local system time (mockable in tests by setting local time, or converting UTC to local)
            // Wait, to make tests predictable and avoid depending on system clock in matches_schedule,
            // we should convert the passed now_utc to Local timezone for local mode!
            let local_time = now_utc.with_timezone(&Local);
            day_str = local_time.format("%a").to_string();
            hour = local_time.hour();
            minute = local_time.minute();
        }

        if !entry.days.is_empty() {
            let matched_day = entry
                .days
                .iter()
                .any(|d| d.to_lowercase() == day_str.to_lowercase());
            if !matched_day {
                return false;
            }
        }

        let current_mins = hour * 60 + minute;
        let from_mins = match Self::parse_time(&entry.from) {
            Some(m) => m,
            None => return false,
        };
        let to_mins = match Self::parse_time(&entry.to) {
            Some(m) => m,
            None => return false,
        };

        if from_mins <= to_mins {
            current_mins >= from_mins && current_mins < to_mins
        } else {
            current_mins >= from_mins || current_mins < to_mins
        }
    }

    pub fn next_transition(
        config: &BandwidthConfig,
        now_utc: DateTime<Utc>,
    ) -> Option<DateTime<Utc>> {
        if config.schedule.is_empty() {
            return None;
        }

        let mut next_trans: Option<DateTime<Utc>> = None;

        for entry in &config.schedule {
            if let Some(ref tz_str) = entry.timezone {
                if let Ok(tz) = tz_str.parse::<chrono_tz::Tz>() {
                    let local_now = now_utc.with_timezone(&tz);
                    for day_offset in 0..=2 {
                        let check_day = local_now + chrono::Duration::days(day_offset);
                        for time_str in &[&entry.from, &entry.to] {
                            if let Some(mins) = Self::parse_time(time_str) {
                                let hr = mins / 60;
                                let min = mins % 60;
                                if let Some(candidate_utc) = tz
                                    .with_ymd_and_hms(
                                        check_day.year(),
                                        check_day.month(),
                                        check_day.day(),
                                        hr,
                                        min,
                                        0,
                                    )
                                    .single()
                                    .map(|dt| dt.with_timezone(&Utc))
                                {
                                    if candidate_utc > now_utc {
                                        next_trans = match next_trans {
                                            None => Some(candidate_utc),
                                            Some(current_best) => {
                                                Some(current_best.min(candidate_utc))
                                            }
                                        };
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                let local_now = now_utc.with_timezone(&Local);
                for day_offset in 0..=2 {
                    let check_day = local_now + chrono::Duration::days(day_offset);
                    for time_str in &[&entry.from, &entry.to] {
                        if let Some(mins) = Self::parse_time(time_str) {
                            let hr = mins / 60;
                            let min = mins % 60;
                            if let Some(candidate_utc) = Local
                                .with_ymd_and_hms(
                                    check_day.year(),
                                    check_day.month(),
                                    check_day.day(),
                                    hr,
                                    min,
                                    0,
                                )
                                .single()
                                .map(|dt| dt.with_timezone(&Utc))
                            {
                                if candidate_utc > now_utc {
                                    next_trans = match next_trans {
                                        None => Some(candidate_utc),
                                        Some(current_best) => Some(current_best.min(candidate_utc)),
                                    };
                                }
                            }
                        }
                    }
                }
            }
        }

        next_trans
    }

    fn parse_time(time_str: &str) -> Option<u32> {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 2 {
            return None;
        }
        let hr = parts[0].parse::<u32>().ok()?;
        let min = parts[1].parse::<u32>().ok()?;
        if hr < 24 && min < 60 {
            Some(hr * 60 + min)
        } else {
            None
        }
    }
}

#[cfg(test)]
#[path = "scheduler_tests.rs"]
mod tests;
