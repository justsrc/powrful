use std::ops::Deref;

use serde::{Deserialize, Serialize};

macro_rules! with_repr {
    ($(
        #[out, $($out:meta),*]
        #[repr, $repr:meta]
        #[$($meta:meta),*]
        $item:item
    )*) => {
        $(
            $(#[$meta])*
            $(#[$out])*
            $item
        )*

        pub mod repr {
            use super::*;
            $(
                $(#[$meta])*
                #[$repr]
                $item
            )*
        }
    };
}

with_repr! {
    #[out, serde(rename_all = "camelCase"), cfg_attr(feature = "specta", derive(specta::Type))]
    #[repr, serde(rename_all(deserialize = "PascalCase", serialize = "camelCase"))]
    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct IORegistryDiagnostic {
        pub diagnostics: Diagnostics,
    }

    #[out, serde(rename_all = "camelCase"), cfg_attr(feature = "specta", derive(specta::Type))]
    #[repr, serde(rename_all(deserialize = "PascalCase", serialize = "camelCase"))]
    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct Diagnostics {
        #[serde(rename = "IORegistry")]
        pub ioregistry: IORegistry,
    }

    #[out, serde(rename_all = "camelCase"), cfg_attr(feature = "specta", derive(specta::Type))]
    #[repr, serde(rename_all(deserialize = "PascalCase", serialize = "camelCase"))]
    #[derive(Debug, Clone, Default, Deserialize, Serialize)]
    pub struct AdapterDetails {
        pub adapter_voltage: Option<i32>,
        pub is_wireless: Option<bool>,
        pub watts: Option<i32>,
        pub name: Option<String>,
        pub current: Option<i32>,
        pub description: Option<String>,
    }


    #[out, serde(rename_all = "camelCase"), cfg_attr(feature = "specta", derive(specta::Type))]
    #[repr, serde(rename_all(deserialize = "PascalCase", serialize = "camelCase"))]
    #[derive(Debug, Clone, Default, Deserialize, Serialize)]
    pub struct PowerTelemetryData {
        pub adapter_efficiency_loss: i32,
        pub battery_power: i64,
        pub system_current_in: i32,
        pub system_energy_consumed: i64,
        pub system_load: i64,
        pub system_power_in: i32,
        pub system_voltage_in: i32,
    }

    #[out, serde(rename_all = "camelCase"), cfg_attr(feature = "specta", derive(specta::Type))]
    #[repr, serde(rename_all(deserialize = "PascalCase", serialize = "camelCase"))]
    #[derive(Debug, Clone, Default, Deserialize, Serialize)]
    pub struct BatteryData {
        pub full_charge_capacity: Option<i32>,
        pub design_capacity: Option<i32>,
        pub remaining_capacity: Option<i32>,
    }

    #[out, serde(rename_all = "camelCase"), cfg_attr(feature = "specta", derive(specta::Type))]
    #[repr, serde(default, rename_all(deserialize = "PascalCase", serialize = "camelCase"))]
    #[derive(Debug, Clone, Default, Deserialize, Serialize)]
    pub struct IORegistry {
        pub adapter_details: AdapterDetails,
        pub battery_data: Option<BatteryData>,
        pub power_telemetry_data: Option<PowerTelemetryData>,
        pub absolute_capacity: i32,
        pub amperage: i32,
        pub voltage: i32,
        pub apple_raw_battery_voltage: Option<i32>,
        pub apple_raw_current_capacity: Option<i32>,
        pub apple_raw_max_capacity: i32,
        pub current_capacity: i32,
        pub cycle_count: i32,
        pub design_capacity: i32,
        pub fully_charged: bool,
        pub instant_amperage: i32,
        pub is_charging: bool,
        pub max_capacity: i32,
        pub temperature: i32,
        pub time_remaining: i32,
        // TODO: check
        pub update_time: i64,
    }
}

impl From<repr::IORegistry> for IORegistry {
    fn from(value: repr::IORegistry) -> Self {
        Self {
            adapter_details: AdapterDetails {
                adapter_voltage: value.adapter_details.adapter_voltage,
                is_wireless: value.adapter_details.is_wireless,
                watts: value.adapter_details.watts,
                name: value.adapter_details.name,
                current: value.adapter_details.current,
                description: value.adapter_details.description,
            },
            battery_data: value.battery_data.map(|data| BatteryData {
                full_charge_capacity: data.full_charge_capacity,
                design_capacity: data.design_capacity,
                remaining_capacity: data.remaining_capacity,
            }),
            power_telemetry_data: value.power_telemetry_data.map(|data| PowerTelemetryData {
                adapter_efficiency_loss: data.adapter_efficiency_loss,
                battery_power: data.battery_power,
                system_current_in: data.system_current_in,
                system_energy_consumed: data.system_energy_consumed,
                system_load: data.system_load,
                system_power_in: data.system_power_in,
                system_voltage_in: data.system_voltage_in,
            }),
            absolute_capacity: value.absolute_capacity,
            amperage: value.amperage,
            voltage: value.voltage,
            apple_raw_battery_voltage: value.apple_raw_battery_voltage,
            apple_raw_current_capacity: value.apple_raw_current_capacity,
            apple_raw_max_capacity: value.apple_raw_max_capacity,
            current_capacity: value.current_capacity,
            cycle_count: value.cycle_count,
            design_capacity: value.design_capacity,
            fully_charged: value.fully_charged,
            instant_amperage: value.instant_amperage,
            is_charging: value.is_charging,
            max_capacity: value.max_capacity,
            temperature: value.temperature,
            time_remaining: value.time_remaining,
            update_time: value.update_time,
        }
    }
}

impl Deref for IORegistry {
    type Target = Option<PowerTelemetryData>;
    fn deref(&self) -> &Self::Target {
        &self.power_telemetry_data
    }
}

impl IORegistry {
    pub fn ptd(&self) -> Option<&PowerTelemetryData> {
        self.power_telemetry_data.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::{repr, IORegistry};

    fn parse_registry(entries: &str) -> Result<repr::IORegistry, plist::Error> {
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <plist version="1.0"><dict>{entries}</dict></plist>"#
        );
        plist::from_bytes(xml.as_bytes())
    }

    #[test]
    fn ioregistry_defaults_missing_fields() {
        let empty = parse_registry("").unwrap();
        assert_eq!(
            plist::to_value(&empty).unwrap(),
            plist::to_value(&repr::IORegistry::default()).unwrap()
        );
        assert_eq!(
            plist::to_value(&IORegistry::from(empty)).unwrap(),
            plist::to_value(&IORegistry::default()).unwrap()
        );

        let partial = parse_registry(
            "<key>Voltage</key><integer>12000</integer><key>IsCharging</key><true/>",
        )
        .unwrap();
        let expected = IORegistry {
            voltage: 12000,
            is_charging: true,
            ..IORegistry::default()
        };
        assert_eq!(
            plist::to_value(&IORegistry::from(partial)).unwrap(),
            plist::to_value(&expected).unwrap()
        );
    }

    #[test]
    fn ioregistry_preserves_raw_current_capacity_presence() {
        for (entries, expected) in [
            ("", None),
            (
                "<key>AppleRawCurrentCapacity</key><integer>0</integer>",
                Some(0),
            ),
        ] {
            let source = parse_registry(entries).unwrap();
            assert_eq!(source.apple_raw_current_capacity, expected);
            assert_eq!(
                IORegistry::from(source).apple_raw_current_capacity,
                expected
            );
        }
    }

    #[test]
    fn ioregistry_rejects_malformed_types() {
        for entries in [
            "<key>Voltage</key><string>12000</string>",
            "<key>IsCharging</key><integer>1</integer>",
            "<key>UpdateTime</key><string>123</string>",
            "<key>AppleRawBatteryVoltage</key><string>12000</string>",
            "<key>AppleRawCurrentCapacity</key><string>0</string>",
            "<key>AdapterDetails</key><string>adapter</string>",
            "<key>AdapterDetails</key><dict><key>Watts</key><string>96</string></dict>",
            "<key>AdapterDetails</key><dict><key>IsWireless</key><integer>1</integer></dict>",
            "<key>AdapterDetails</key><dict><key>Name</key><integer>1</integer></dict>",
            "<key>BatteryData</key><string>battery</string>",
            "<key>BatteryData</key><integer>1</integer>",
            "<key>BatteryData</key><dict><key>FullChargeCapacity</key><string>8322</string></dict>",
            "<key>BatteryData</key><dict><key>DesignCapacity</key><true/></dict>",
            "<key>BatteryData</key><dict><key>RemainingCapacity</key><string>0</string></dict>",
            "<key>PowerTelemetryData</key><string>telemetry</string>",
            "<key>PowerTelemetryData</key><dict>
                <key>AdapterEfficiencyLoss</key><integer>1</integer>
                <key>BatteryPower</key><string>2</string>
                <key>SystemCurrentIn</key><integer>3</integer>
                <key>SystemEnergyConsumed</key><integer>4</integer>
                <key>SystemLoad</key><integer>5</integer>
                <key>SystemPowerIn</key><integer>6</integer>
                <key>SystemVoltageIn</key><integer>7</integer>
            </dict>",
        ] {
            assert!(parse_registry(entries).is_err(), "accepted: {entries}");
        }
    }

    #[test]
    fn ioregistry_defaults_do_not_relax_other_structs() {
        assert!(parse_registry("<key>PowerTelemetryData</key><dict/>").is_err());
        let empty = br#"<?xml version="1.0" encoding="UTF-8"?>
            <plist version="1.0"><dict/></plist>"#;
        assert!(plist::from_bytes::<IORegistry>(empty).is_err());
        assert!(plist::from_bytes::<repr::Diagnostics>(empty).is_err());
        assert!(plist::from_bytes::<repr::IORegistryDiagnostic>(empty).is_err());
    }

    #[test]
    fn ioregistry_conversion_preserves_nonzero_and_nested_fields() {
        let source = parse_registry(
            "<key>AdapterDetails</key><dict>
                <key>AdapterVoltage</key><integer>20000</integer>
                <key>IsWireless</key><true/>
                <key>Watts</key><integer>96</integer>
                <key>Name</key><string>Synthetic adapter</string>
                <key>Current</key><integer>4800</integer>
                <key>Description</key><string>Test adapter description</string>
            </dict>
            <key>PowerTelemetryData</key><dict>
                <key>AdapterEfficiencyLoss</key><integer>101</integer>
                <key>BatteryPower</key><integer>-5000000001</integer>
                <key>SystemCurrentIn</key><integer>303</integer>
                <key>SystemEnergyConsumed</key><integer>6000000002</integer>
                <key>SystemLoad</key><integer>7000000003</integer>
                <key>SystemPowerIn</key><integer>606</integer>
                <key>SystemVoltageIn</key><integer>707</integer>
            </dict>
            <key>AbsoluteCapacity</key><integer>4101</integer>
            <key>Amperage</key><integer>-1202</integer>
            <key>Voltage</key><integer>12003</integer>
            <key>AppleRawBatteryVoltage</key><integer>12004</integer>
            <key>AppleRawCurrentCapacity</key><integer>4205</integer>
            <key>AppleRawMaxCapacity</key><integer>5106</integer>
            <key>CurrentCapacity</key><integer>87</integer>
            <key>CycleCount</key><integer>208</integer>
            <key>DesignCapacity</key><integer>6009</integer>
            <key>FullyCharged</key><true/>
            <key>InstantAmperage</key><integer>-1310</integer>
            <key>IsCharging</key><true/>
            <key>MaxCapacity</key><integer>5111</integer>
            <key>Temperature</key><integer>3012</integer>
            <key>TimeRemaining</key><integer>113</integer>
            <key>UpdateTime</key><integer>8000000014</integer>",
        )
        .unwrap();
        let converted = IORegistry::from(source);

        assert_eq!(converted.absolute_capacity, 4101);
        assert_eq!(converted.amperage, -1202);
        assert_eq!(converted.voltage, 12003);
        assert_eq!(converted.apple_raw_battery_voltage, Some(12004));
        assert_eq!(converted.apple_raw_current_capacity, Some(4205));
        assert_eq!(converted.apple_raw_max_capacity, 5106);
        assert_eq!(converted.current_capacity, 87);
        assert_eq!(converted.cycle_count, 208);
        assert_eq!(converted.design_capacity, 6009);
        assert!(converted.fully_charged);
        assert_eq!(converted.instant_amperage, -1310);
        assert!(converted.is_charging);
        assert_eq!(converted.max_capacity, 5111);
        assert_eq!(converted.temperature, 3012);
        assert_eq!(converted.time_remaining, 113);
        assert_eq!(converted.update_time, 8000000014);

        let adapter = converted.adapter_details;
        assert_eq!(adapter.adapter_voltage, Some(20000));
        assert_eq!(adapter.is_wireless, Some(true));
        assert_eq!(adapter.watts, Some(96));
        assert_eq!(adapter.name.as_deref(), Some("Synthetic adapter"));
        assert_eq!(adapter.current, Some(4800));
        assert_eq!(
            adapter.description.as_deref(),
            Some("Test adapter description")
        );

        let telemetry = converted.power_telemetry_data.unwrap();
        assert_eq!(telemetry.adapter_efficiency_loss, 101);
        assert_eq!(telemetry.battery_power, -5000000001);
        assert_eq!(telemetry.system_current_in, 303);
        assert_eq!(telemetry.system_energy_consumed, 6000000002);
        assert_eq!(telemetry.system_load, 7000000003);
        assert_eq!(telemetry.system_power_in, 606);
        assert_eq!(telemetry.system_voltage_in, 707);
    }
}
