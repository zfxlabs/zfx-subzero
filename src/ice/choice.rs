use crate::colored::Colorize;

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum Choice {
    Live,
    Faulty,
}

impl std::fmt::Debug for Choice {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Choice::Live => {
                write!(fmt, "{}", "Live".green(),)
            }
            Choice::Faulty => {
                write!(fmt, "{}", "Faulty".red(),)
            }
        }
    }
}
