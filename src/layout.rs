//! EPI2ME Basic QC Tutorial's explicit, row-major PromethION channel map.
//! Source: https://epi2me.nanoporetech.com/notebooks/Basic_QC_Tutorial.html
use clap::ValueEnum;
use serde::Serialize;

#[derive(Clone, Copy, Debug, ValueEnum, Serialize)]
pub enum FlowcellLayout {
    #[value(name = "promethion-3000")]
    #[serde(rename = "promethion-3000")]
    Promethion3000,
}

impl FlowcellLayout {
    pub fn coordinate(self, channel: u32) -> Option<ChannelCoordinate> {
        if !(1..=3000).contains(&channel) {
            return None;
        }
        // np.hstack([np.arange(x, x + 250).reshape(25, 10)
        //            for x in range(1, 2752, 250)])
        let index = channel - 1;
        Some(ChannelCoordinate {
            channel,
            row: (index % 250) / 10,
            column: (index / 250) * 10 + index % 10,
        })
    }

    pub fn description(self) -> PhysicalLayout {
        PhysicalLayout {
            name: self,
            rows: 25,
            columns: 120,
            channel_coordinates: (1..=3000).map(|c| self.coordinate(c).unwrap()).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ChannelCoordinate {
    pub channel: u32,
    pub row: u32,
    pub column: u32,
}

#[derive(Debug, Serialize)]
pub struct PhysicalLayout {
    pub name: FlowcellLayout,
    pub rows: u32,
    pub columns: u32,
    pub channel_coordinates: Vec<ChannelCoordinate>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn reference_block_boundaries_match_numpy_row_major_layout() {
        let layout = FlowcellLayout::Promethion3000;
        for (channel, row, column) in [
            (1, 0, 0),
            (10, 0, 9),
            (11, 1, 0),
            (250, 24, 9),
            (251, 0, 10),
            (3000, 24, 119),
        ] {
            let actual = layout.coordinate(channel).unwrap();
            assert_eq!((actual.row, actual.column), (row, column));
        }
        assert!(layout.coordinate(0).is_none());
        assert!(layout.coordinate(3001).is_none());
    }

    #[test]
    fn every_channel_has_one_unique_in_range_coordinate() {
        let map = FlowcellLayout::Promethion3000.description();
        let mut coordinates = BTreeSet::new();
        for c in &map.channel_coordinates {
            assert!(c.row < map.rows && c.column < map.columns);
            assert!(coordinates.insert((c.row, c.column)));
            // Independent transcription of the reference's row-major matrix.
            assert_eq!(
                c.channel,
                (c.column / 10) * 250 + c.row * 10 + c.column % 10 + 1
            );
        }
        assert_eq!(coordinates.len(), 3000);
        assert_eq!(map.channel_coordinates.len(), 3000);
    }
}
