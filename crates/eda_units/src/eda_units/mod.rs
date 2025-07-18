//! Design References:
//!
//! | IPC Std        | Purpose                          | Min Precision (mm) | Inches (")            | Mils (thou)         | Chinese Si (丝) | Verification Sources |
//! |----------------|----------------------------------|--------------------|-----------------------|---------------------|-----------------|----------------------|
//! | **IPC-7351**   | SMT land patterns                | 0.005 mm           | 0.00019685"           | 0.19685 mils        | 0.5 Si          | [IPC-7351B §5.3](https://www.ipc.org/TOC/IPC-7351B.pdf) |
//! | **IPC-2221A**  | PCB design (traces/vias)         | 0.01–0.025 mm      | 0.0003937–0.000984"   | 0.3937–0.984 mils   | 1–2.5 Si        | [IPC-2221A §6.1](https://www.ipc.org/TOC/IPC-2221A.pdf) |
//! | **IPC-2615**   | Fabrication tolerances           | 0.005 mm           | 0.00019685"           | 0.19685 mils        | 0.5 Si          | [IPC-2615 §4.2](https://www.ipc.org/4.0_Knowledge/4.1_Standards/ipc-2615.pdf) |
//! | **IPC-2581C**  | Manufacturing data exchange      | 0.0005 mm          | 0.000019685"          | 0.019685 mils       | 0.05 Si         | [IPC-2581C §3.8](https://www.ipc.org/TOC/IPC-2581C.pdf) |
//! | **IPC-D-356**  | Bare-board test data             | 0.005–0.01 mm      | 0.00019685–0.0003937" | 0.19685–0.3937 mils | 0.5–1 Si        | [IPC-D-356D §5.2](https://www.ipc.org/TOC/IPC-D-356D.pdf) |

mod dimension;
mod dimension_unit;
mod unit_system;

#[cfg(test)]
mod example_usage_tests {
    use nalgebra::Point2;

    use crate::eda_units::dimension_unit::{DimensionUnit, DimensionUnitPoint2Ext};
    use crate::eda_units::unit_system::UnitSystem;

    #[test]
    fn test_typical_pcb_scenario() {
        // Demonstrating PCB design values with appropriate precision

        // Via diameter in mils
        let via_diameter = DimensionUnit::from_f64(12.0, UnitSystem::Mils);
        assert_eq!(format!("{}", via_diameter), "12.000 mil");

        // Trace width in mils
        let trace_width = DimensionUnit::from_f64(6.0, UnitSystem::Mils);
        assert_eq!(format!("{}", trace_width), "6.000 mil");

        // Board dimensions in inches
        let board_width = DimensionUnit::from_f64(2.5, UnitSystem::Inches);
        let board_height = DimensionUnit::from_f64(1.75, UnitSystem::Inches);
        assert_eq!(format!("{}", board_width), "2.500000 in");
        assert_eq!(format!("{}", board_height), "1.750000 in");

        // Component placement coordinates in millimeters
        let component_position = Point2::new_dim_f64(25.4, 50.8, UnitSystem::Millimeters);
        assert_eq!(component_position.display(), "(25.4000, 50.8000) mm");

        // Converting between units maintains appropriate precision
        let via_in_mm = via_diameter.in_unit_system(UnitSystem::Millimeters);
        assert_eq!(format!("{}", via_in_mm), "0.3048 mm");

        let board_in_mm = board_width.in_unit_system(UnitSystem::Millimeters);
        assert_eq!(format!("{}", board_in_mm), "63.5000 mm");
    }

    #[test]
    fn test_ipc_spec_compliance() {
        // Test showing compliance with IPC specification precision requirements

        // IPC Class 2 pad dimensions in inches (high precision)
        let pad_width = DimensionUnit::from_f64(0.063, UnitSystem::Inches);
        let pad_height = DimensionUnit::from_f64(0.025, UnitSystem::Inches);

        assert_eq!(format!("{}", pad_width), "0.063000 in");
        assert_eq!(format!("{}", pad_height), "0.025000 in");

        // Convert to mm for manufacturing files (4 decimal places)
        let pad_width_mm = pad_width.in_unit_system(UnitSystem::Millimeters);
        let pad_height_mm = pad_height.in_unit_system(UnitSystem::Millimeters);

        assert_eq!(format!("{}", pad_width_mm), "1.6002 mm");
        assert_eq!(format!("{}", pad_height_mm), "0.6350 mm");

        // Convert to mils for design guidelines (3 decimal places)
        let pad_width_mils = pad_width.in_unit_system(UnitSystem::Mils);
        let pad_height_mils = pad_height.in_unit_system(UnitSystem::Mils);

        assert_eq!(format!("{}", pad_width_mils), "63.000 mil");
        assert_eq!(format!("{}", pad_height_mils), "25.000 mil");
    }
}
