use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase { Prepared, Copied, Swapped, PreviousSaved, Respawned, Done, RollbackPrepared, RolledBack }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observed {
    pub current: bool,
    pub staged: bool,
    pub previous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action { DiscardStaged, ResumeSwap, SavePrevious, RespawnCurrent, Finish, RestorePrevious, Fail }

pub fn classify(phase: Phase, seen: Observed) -> Action {
    use Action::*;
    use Phase::*;
    if !seen.current { return if seen.previous { RestorePrevious } else { Fail }; }
    match phase {
        Prepared => if seen.staged { ResumeSwap } else { DiscardStaged },
        Copied => if seen.staged { ResumeSwap } else { Fail },
        Swapped => if seen.staged { SavePrevious } else if seen.previous { RespawnCurrent } else { Fail },
        PreviousSaved => if seen.previous { RespawnCurrent } else { Fail },
        Respawned | Done | RolledBack => Finish,
        RollbackPrepared => if seen.previous { RestorePrevious } else { Fail },
    }
}

pub fn observe(current: &std::path::Path, staged: &std::path::Path,
    previous: &std::path::Path) -> Observed {
    Observed { current: current.exists(), staged: staged.exists(), previous: previous.exists() }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_phase_and_disk_shape_is_classified() {
        let phases = [Phase::Prepared, Phase::Copied, Phase::Swapped,
            Phase::PreviousSaved, Phase::Respawned, Phase::Done,
            Phase::RollbackPrepared, Phase::RolledBack];
        let mut count = 0;
        for phase in phases {
            for bits in 0..8 {
                let _named_action = classify(phase, Observed {
                    current: bits & 1 != 0, staged: bits & 2 != 0,
                    previous: bits & 4 != 0,
                });
                count += 1;
            }
        }
        assert_eq!(count, 64);
    }

    #[test]
    fn every_crash_shape_can_be_classified_from_disk() {
        let root = std::env::temp_dir().join(format!("strop-recovery-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root); std::fs::create_dir_all(&root).unwrap();
        let current = root.join("current"); let staged = root.join("staged");
        let previous = root.join("previous");
        for bits in 0..8 {
            for path in [&current, &staged, &previous] { let _ = std::fs::remove_file(path); }
            if bits & 1 != 0 { std::fs::write(&current, b"x").unwrap(); }
            if bits & 2 != 0 { std::fs::write(&staged, b"x").unwrap(); }
            if bits & 4 != 0 { std::fs::write(&previous, b"x").unwrap(); }
            let seen = observe(&current, &staged, &previous);
            for phase in [Phase::Prepared, Phase::Copied, Phase::Swapped,
                Phase::PreviousSaved, Phase::Respawned, Phase::Done,
                Phase::RollbackPrepared, Phase::RolledBack] {
                let _named = classify(phase, seen);
            }
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
