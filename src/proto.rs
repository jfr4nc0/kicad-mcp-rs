#![allow(clippy::all, dead_code)]

pub mod v10 {
    pub mod kiapi {
        pub mod common {
            include!(concat!(env!("OUT_DIR"), "/kicad10/kiapi.common.rs"));
            pub mod commands {
                include!(concat!(
                    env!("OUT_DIR"),
                    "/kicad10/kiapi.common.commands.rs"
                ));
            }
            pub mod project {
                include!(concat!(env!("OUT_DIR"), "/kicad10/kiapi.common.project.rs"));
            }
            pub mod types {
                include!(concat!(env!("OUT_DIR"), "/kicad10/kiapi.common.types.rs"));
            }
        }
        pub mod board {
            include!(concat!(env!("OUT_DIR"), "/kicad10/kiapi.board.rs"));
            pub mod commands {
                include!(concat!(env!("OUT_DIR"), "/kicad10/kiapi.board.commands.rs"));
            }
            pub mod types {
                include!(concat!(env!("OUT_DIR"), "/kicad10/kiapi.board.types.rs"));
            }
        }
        pub mod schematic {
            pub mod types {
                include!(concat!(
                    env!("OUT_DIR"),
                    "/kicad10/kiapi.schematic.types.rs"
                ));
            }
        }
    }
}

pub mod v11 {
    pub mod kiapi {
        pub mod common {
            include!(concat!(env!("OUT_DIR"), "/kicad11/kiapi.common.rs"));
            pub mod commands {
                include!(concat!(
                    env!("OUT_DIR"),
                    "/kicad11/kiapi.common.commands.rs"
                ));
            }
            pub mod project {
                include!(concat!(env!("OUT_DIR"), "/kicad11/kiapi.common.project.rs"));
            }
            pub mod types {
                include!(concat!(env!("OUT_DIR"), "/kicad11/kiapi.common.types.rs"));
            }
        }
        pub mod board {
            include!(concat!(env!("OUT_DIR"), "/kicad11/kiapi.board.rs"));
            pub mod commands {
                include!(concat!(env!("OUT_DIR"), "/kicad11/kiapi.board.commands.rs"));
            }
            pub mod jobs {
                include!(concat!(env!("OUT_DIR"), "/kicad11/kiapi.board.jobs.rs"));
            }
            pub mod types {
                include!(concat!(env!("OUT_DIR"), "/kicad11/kiapi.board.types.rs"));
            }
        }
        pub mod schematic {
            include!(concat!(env!("OUT_DIR"), "/kicad11/kiapi.schematic.rs"));
            pub mod commands {
                include!(concat!(
                    env!("OUT_DIR"),
                    "/kicad11/kiapi.schematic.commands.rs"
                ));
            }
            pub mod jobs {
                include!(concat!(env!("OUT_DIR"), "/kicad11/kiapi.schematic.jobs.rs"));
            }
            pub mod types {
                include!(concat!(
                    env!("OUT_DIR"),
                    "/kicad11/kiapi.schematic.types.rs"
                ));
            }
        }
    }
}
