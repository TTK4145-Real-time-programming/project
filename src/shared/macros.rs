/***************************************/
/*               Macros                */
/***************************************/
#[macro_export]
macro_rules! unwrap_or_exit {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                eprintln!("ERROR: {}", e);
                std::process::exit(1);
            }
        }
    };
}
