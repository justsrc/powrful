use std::{
    collections::VecDeque,
    ffi::CString,
    mem,
    ops::{Deref, Div},
    time::Duration,
};

use anyhow::bail;
use core_foundation::{
    base::{kCFAllocatorDefault, mach_port_t, TCFType},
    dictionary::{CFDictionary, CFMutableDictionaryRef},
};
use derive_more::Add;
use io_kit_sys::{
    ret::kIOReturnSuccess, IOMasterPort, IORegistryEntryCreateCFProperties,
    IOServiceGetMatchingService, IOServiceMatching,
};
use ratatui::widgets::SparklineBar;
use serde::{Deserialize, Serialize};

use crate::{
    de::{repr, IORegistry},
    ffi::{smc::SMCPowerData, InterfaceType},
    util::{dict_into, skip_until},
};

pub mod remote;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NormalizedResource {
    pub is_local: bool,
    pub is_charging: bool,
    pub time_remain: Duration,
    pub last_update: i64,
    pub adapter_name: Option<String>,
    pub cycle_count: i32,
    pub current_capacity: Option<i32>,
    pub max_capacity: i32,
    #[serde(default)]
    pub design_capacity: i32,
    #[serde(flatten)]
    pub data: NormalizedData,
}

#[derive(Debug, Clone, Copy, Default, Add, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NormalizedData {
    pub system_in: f32,
    pub system_load: f32,
    pub battery_power: f32,
    pub adapter_power: f32,
    pub efficiency_loss: f32,
    /// 0 if not available
    pub brightness_power: f32,
    /// 0 if not available
    pub heatpipe_power: f32,
    pub battery_level: i32,
    pub absolute_battery_level: f32,
    pub temperature: f32,

    pub adapter_watts: f32,
    pub adapter_voltage: f32,
    pub adapter_amperage: f32,
}

impl NormalizedData {
    pub fn max_with(self, other: &Self) -> Self {
        Self {
            system_in: self.system_in.max(other.system_in),
            system_load: self.system_load.max(other.system_load),
            battery_power: self.battery_power.max(other.battery_power),
            adapter_power: self.adapter_power.max(other.adapter_power),
            efficiency_loss: self.efficiency_loss.max(other.efficiency_loss),
            battery_level: self.battery_level.max(other.battery_level),
            absolute_battery_level: self
                .absolute_battery_level
                .max(other.absolute_battery_level),
            temperature: self.temperature.max(other.temperature),
            brightness_power: self.brightness_power.max(other.brightness_power),
            heatpipe_power: self.heatpipe_power.max(other.heatpipe_power),
            adapter_watts: self.adapter_watts.max(other.adapter_watts),
            adapter_voltage: self.adapter_voltage.max(other.adapter_voltage),
            adapter_amperage: self.adapter_amperage.max(other.adapter_amperage),
        }
    }
}

impl Div<f32> for NormalizedData {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self {
            system_in: self.system_in / rhs,
            system_load: self.system_load / rhs,
            battery_power: self.battery_power / rhs,
            adapter_power: self.adapter_power / rhs,
            efficiency_loss: self.efficiency_loss / rhs,
            brightness_power: self.brightness_power / rhs,
            heatpipe_power: self.heatpipe_power / rhs,
            battery_level: self.battery_level / rhs as i32,
            absolute_battery_level: self.absolute_battery_level / rhs,
            temperature: self.temperature / rhs,
            adapter_watts: self.adapter_watts / rhs,
            adapter_voltage: self.adapter_voltage / rhs,
            adapter_amperage: self.adapter_amperage / rhs,
        }
    }
}

impl Deref for NormalizedResource {
    type Target = NormalizedData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

struct ResolvedCapacities {
    current: Option<i32>,
    max: i32,
    design: i32,
    percentage: f32,
}

fn resolved_capacities(io: &IORegistry) -> ResolvedCapacities {
    let battery = io.battery_data.as_ref();
    let current = battery
        .and_then(|data| data.remaining_capacity)
        .filter(|&capacity| capacity >= 0)
        .or(io
            .apple_raw_current_capacity
            .filter(|&capacity| capacity >= 0));
    let max = battery
        .and_then(|data| data.full_charge_capacity)
        .filter(|&capacity| capacity > 0)
        .unwrap_or(io.apple_raw_max_capacity);
    let design = battery
        .and_then(|data| data.design_capacity)
        .filter(|&capacity| capacity > 0)
        .unwrap_or(io.design_capacity);
    let percentage = match current {
        Some(current) if max > 0 => current as f32 / max as f32 * 100.,
        _ => io.current_capacity as f32,
    };

    ResolvedCapacities {
        current,
        max,
        design,
        percentage,
    }
}

impl From<&IORegistry> for NormalizedResource {
    fn from(io: &IORegistry) -> Self {
        let capacities = resolved_capacities(io);
        let (system_in, system_load, battery_power, adapter_power, efficiency_loss) =
            if let Some(d) = io.ptd() {
                (
                    d.system_power_in as f32 / 1000.,
                    d.system_load as f32 / 1000.,
                    d.battery_power as f32 / 1000.,
                    (d.system_power_in + d.adapter_efficiency_loss) as f32 / 1000.,
                    d.adapter_efficiency_loss as f32 / 1000.,
                )
            } else {
                Default::default()
            };

        Self {
            is_local: false,
            is_charging: io.is_charging,
            time_remain: Duration::from_secs(io.time_remaining as u64 * 60),
            last_update: io.update_time,
            adapter_name: io
                .adapter_details
                .name
                .clone()
                .or_else(|| io.adapter_details.description.clone()),
            cycle_count: io.cycle_count,
            max_capacity: capacities.max,
            design_capacity: capacities.design,
            current_capacity: capacities.current,
            data: NormalizedData {
                system_in,
                system_load,
                battery_power,
                adapter_power,
                efficiency_loss,
                brightness_power: 0.,
                heatpipe_power: 0.,
                battery_level: io.current_capacity,
                absolute_battery_level: capacities.percentage,
                temperature: io.temperature as f32 / 100.,

                adapter_watts: io.adapter_details.watts.unwrap_or_default() as f32,
                adapter_voltage: io.adapter_details.adapter_voltage.unwrap_or_default() as f32
                    / 1000.,
                adapter_amperage: io.adapter_details.current.unwrap_or_default() as f32 / 1000.,
            },
        }
    }
}

impl From<(&IORegistry, &SMCPowerData)> for NormalizedResource {
    fn from((io, smc): (&IORegistry, &SMCPowerData)) -> Self {
        let capacities = resolved_capacities(io);

        Self {
            is_local: true,
            last_update: io.update_time,
            is_charging: smc.is_charging(),
            time_remain: Duration::from_secs_f32(
                60.0 * if smc.is_charging() {
                    smc.time_to_full
                } else {
                    smc.time_to_empty
                },
            ),
            adapter_name: io
                .adapter_details
                .name
                .clone()
                .or_else(|| io.adapter_details.description.clone()),
            cycle_count: io.cycle_count,
            max_capacity: capacities.max,
            design_capacity: capacities.design,
            current_capacity: capacities.current,
            data: NormalizedData {
                system_in: smc.delivery_rate,
                system_load: smc.system_total,
                battery_power: smc.battery_rate.max(smc.delivery_rate - smc.system_total),
                efficiency_loss: io
                    .ptd()
                    .map_or(0.0, |d| d.adapter_efficiency_loss as f32 / 1000.),
                brightness_power: smc.brightness,
                heatpipe_power: smc.heatpipe,
                battery_level: io.current_capacity,
                absolute_battery_level: capacities.percentage,
                temperature: smc.temperature,
                adapter_power: smc.delivery_rate
                    + io.ptd()
                        .map_or(0.0, |d| d.adapter_efficiency_loss as f32 / 1000.),

                adapter_watts: io.adapter_details.watts.unwrap_or_default() as f32,
                adapter_voltage: io.adapter_details.adapter_voltage.unwrap_or_default() as f32
                    / 1000.,
                adapter_amperage: io.adapter_details.current.unwrap_or_default() as f32 / 1000.,
            },
        }
    }
}

pub fn get_mac_ioreg_dict() -> anyhow::Result<CFDictionary> {
    let mut master_port: mach_port_t = 0;
    if unsafe { IOMasterPort(0, &mut master_port) } != 0 {
        bail!("could not get master port");
    }
    let name = CString::new("AppleSmartBattery").unwrap();
    let matching_dict = unsafe { IOServiceMatching(name.as_ptr()) };

    let result = unsafe { IOServiceGetMatchingService(master_port, matching_dict) };

    let mut properties: CFMutableDictionaryRef = unsafe { mem::zeroed() };
    if unsafe { IORegistryEntryCreateCFProperties(result, &mut properties, kCFAllocatorDefault, 0) }
        != kIOReturnSuccess
    {
        bail!("could not get properties");
    }

    unsafe { Ok(CFDictionary::wrap_under_create_rule(properties)) }
}

pub fn get_mac_ioreg() -> anyhow::Result<IORegistry> {
    let dic = get_mac_ioreg_dict()?;
    parse_mac_ioreg(dic)
}

fn parse_mac_ioreg(dic: CFDictionary) -> anyhow::Result<IORegistry> {
    Ok(dict_into::<repr::IORegistry>(dic)?.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::DictParseError;

    #[test]
    fn malformed_registry_returns_a_formattable_error() {
        let dictionary = crate::cfdic! { "Voltage" = "invalid" }.to_untyped();
        let error = parse_mac_ioreg(dictionary).unwrap_err();
        assert!(error.downcast_ref::<DictParseError>().is_some());
        assert!(format!("{error:?}").contains("Failed to parse plist"));
    }

    fn assert_capacity_normalizations(
        entries: &str,
        nested: Option<(Option<i32>, Option<i32>, Option<i32>)>,
        expected: (Option<i32>, i32, i32, f32),
    ) {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <plist version="1.0"><dict>
                <key>CurrentCapacity</key><integer>73</integer>
                {entries}
            </dict></plist>"#
        );
        let source: repr::IORegistry = plist::from_bytes(xml.as_bytes()).unwrap();
        assert_eq!(
            source.battery_data.as_ref().map(|data| (
                data.full_charge_capacity,
                data.design_capacity,
                data.remaining_capacity,
            )),
            nested
        );
        let io = IORegistry::from(source);
        assert_eq!(
            io.battery_data.as_ref().map(|data| (
                data.full_charge_capacity,
                data.design_capacity,
                data.remaining_capacity,
            )),
            nested
        );
        let smc = SMCPowerData::default();
        for normalized in [
            NormalizedResource::from(&io),
            NormalizedResource::from((&io, &smc)),
        ] {
            assert_eq!(normalized.current_capacity, expected.0);
            assert_eq!(normalized.max_capacity, expected.1);
            assert_eq!(normalized.design_capacity, expected.2);
            assert_eq!(normalized.battery_level, 73);
            assert!((normalized.absolute_battery_level - expected.3).abs() < 0.0001);
        }
    }

    const LEGACY_CAPACITIES: &str = "
        <key>AppleRawCurrentCapacity</key><integer>4000</integer>
        <key>AppleRawMaxCapacity</key><integer>5000</integer>
        <key>DesignCapacity</key><integer>6000</integer>";

    #[test]
    fn macos27_nested_capacities_without_raw_root() {
        assert_capacity_normalizations(
            "<key>BatteryData</key><dict>
                <key>FullChargeCapacity</key><integer>8322</integer>
                <key>DesignCapacity</key><integer>8694</integer>
                <key>RemainingCapacity</key><integer>7623</integer>
                <key>NominalChargeCapacity</key><integer>8200</integer>
            </dict>",
            Some((Some(8322), Some(8694), Some(7623))),
            (Some(7623), 8322, 8694, 7623. / 8322. * 100.),
        );
    }

    #[test]
    fn legacy_capacities_without_battery_data() {
        assert_capacity_normalizations(LEGACY_CAPACITIES, None, (Some(4000), 5000, 6000, 80.));
    }

    #[test]
    fn partial_nested_capacities_resolve_independently() {
        for (entries, nested, expected) in [
            (
                "<key>FullChargeCapacity</key><integer>8322</integer>",
                (Some(8322), None, None),
                (Some(4000), 8322, 6000, 4000. / 8322. * 100.),
            ),
            (
                "<key>DesignCapacity</key><integer>8694</integer>",
                (None, Some(8694), None),
                (Some(4000), 5000, 8694, 80.),
            ),
            (
                "<key>RemainingCapacity</key><integer>7623</integer>",
                (None, None, Some(7623)),
                (Some(7623), 5000, 6000, 7623. / 5000. * 100.),
            ),
            ("", (None, None, None), (Some(4000), 5000, 6000, 80.)),
        ] {
            assert_capacity_normalizations(
                &format!("{LEGACY_CAPACITIES}<key>BatteryData</key><dict>{entries}</dict>"),
                Some(nested),
                expected,
            );
        }
    }

    #[test]
    fn missing_current_capacity_with_positive_maximum_is_unavailable() {
        assert_capacity_normalizations(
            "<key>AppleRawMaxCapacity</key><integer>5000</integer>
            <key>BatteryData</key><dict>
                <key>FullChargeCapacity</key><integer>8322</integer>
                <key>DesignCapacity</key><integer>8694</integer>
            </dict>",
            Some((Some(8322), Some(8694), None)),
            (None, 8322, 8694, 73.),
        );
    }

    #[test]
    fn explicit_legacy_zero_capacity_is_preserved() {
        assert_capacity_normalizations(
            "<key>AppleRawCurrentCapacity</key><integer>0</integer>
             <key>AppleRawMaxCapacity</key><integer>5000</integer>",
            None,
            (Some(0), 5000, 0, 0.),
        );
    }

    #[test]
    fn missing_or_negative_legacy_current_capacity_is_unavailable() {
        for entries in [
            "",
            "<key>AppleRawCurrentCapacity</key><integer>-1</integer>",
        ] {
            assert_capacity_normalizations(
                &format!("<key>AppleRawMaxCapacity</key><integer>5000</integer>{entries}"),
                None,
                (None, 5000, 0, 73.),
            );
        }
    }

    #[test]
    fn zero_remaining_capacity_overrides_legacy_current() {
        assert_capacity_normalizations(
            &format!(
                "{LEGACY_CAPACITIES}<key>BatteryData</key><dict>
                    <key>FullChargeCapacity</key><integer>0</integer>
                    <key>DesignCapacity</key><integer>0</integer>
                    <key>RemainingCapacity</key><integer>0</integer>
                </dict>"
            ),
            Some((Some(0), Some(0), Some(0))),
            (Some(0), 5000, 6000, 0.),
        );
    }

    #[test]
    fn negative_nested_capacities_fall_back_to_legacy() {
        assert_capacity_normalizations(
            &format!(
                "{LEGACY_CAPACITIES}<key>BatteryData</key><dict>
                    <key>FullChargeCapacity</key><integer>-8322</integer>
                    <key>DesignCapacity</key><integer>-8694</integer>
                    <key>RemainingCapacity</key><integer>-7623</integer>
                </dict>"
            ),
            Some((Some(-8322), Some(-8694), Some(-7623))),
            (Some(4000), 5000, 6000, 80.),
        );
    }

    #[test]
    fn nominal_charge_capacity_is_not_a_maximum() {
        assert_capacity_normalizations(
            "<key>BatteryData</key><dict>
                <key>NominalChargeCapacity</key><integer>8322</integer>
                <key>DesignCapacity</key><integer>8694</integer>
                <key>RemainingCapacity</key><integer>7623</integer>
            </dict>",
            Some((None, Some(8694), Some(7623))),
            (Some(7623), 0, 8694, 73.),
        );
    }

    #[test]
    fn nonpositive_resolved_maximum_uses_reported_percentage() {
        for max in [0, -5000] {
            assert_capacity_normalizations(
                &format!(
                    "<key>AppleRawMaxCapacity</key><integer>{max}</integer>
                    <key>BatteryData</key><dict>
                        <key>FullChargeCapacity</key><integer>-8322</integer>
                        <key>RemainingCapacity</key><integer>7623</integer>
                    </dict>"
                ),
                Some((Some(-8322), None, Some(7623))),
                (Some(7623), max, 0, 73.),
            );
        }
    }

    #[test]
    fn missing_raw_capacity_uses_reported_battery_percentage() {
        let io = IORegistry {
            current_capacity: 73,
            ..Default::default()
        };
        assert_eq!(NormalizedResource::from(&io).absolute_battery_level, 73.);
        assert_eq!(
            NormalizedResource::from((&io, &SMCPowerData::default())).absolute_battery_level,
            73.
        );
    }
}

#[derive(Debug)]
pub struct MergedPowerData {
    pub from: PowerDataFrom,
    pub smc: Option<SMCPowerData>,
    pub ioreg: IORegistry,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PowerDataFrom {
    #[default]
    Local,
    Remote((String, String, InterfaceType)),
}

impl Deref for MergedPowerData {
    type Target = IORegistry;

    fn deref(&self) -> &Self::Target {
        &self.ioreg
    }
}

#[derive(Debug, Default)]
pub struct PowerStatistic {
    pub max_battery_power: f32,
    pub max_input_power: f32,
    pub max_system_power: f32,

    pub battery_history: VecDeque<u64>,
    pub input_history: VecDeque<u64>,
    pub system_history: VecDeque<u64>,
}

impl PowerStatistic {
    pub fn update(&mut self, battery_power: f32, input_power: f32, system_power: f32) {
        if battery_power > self.max_battery_power {
            self.max_battery_power = battery_power;
        }

        if input_power > self.max_input_power {
            self.max_input_power = input_power;
        }

        if system_power > self.max_system_power {
            self.max_system_power = system_power;
        }

        self.battery_history.push_back(battery_power.abs() as u64);
        if self.battery_history.len() > 50 {
            self.battery_history.pop_front();
        }

        self.input_history.push_back(input_power.abs() as u64);
        if self.input_history.len() > 50 {
            self.input_history.pop_front();
        }

        self.system_history.push_back(system_power.abs() as u64);
        if self.system_history.len() > 200 {
            self.system_history.pop_front();
        }
    }

    pub fn battery_history(&self, width: usize) -> Vec<SparklineBar> {
        skip_until(self.battery_history.iter(), width)
            .map(|v| SparklineBar::from(*v))
            .collect()
    }

    pub fn input_history(&self, width: usize) -> Vec<SparklineBar> {
        skip_until(self.input_history.iter(), width)
            .map(|v| SparklineBar::from(*v))
            .collect()
    }

    pub fn system_history(&self, width: usize) -> Vec<SparklineBar> {
        skip_until(self.system_history.iter(), width)
            .map(|v| SparklineBar::from(*v))
            .collect()
    }
}
