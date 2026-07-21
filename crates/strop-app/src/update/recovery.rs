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
    fn every_phase_and_disk_shape_has_the_required_action() {
        use Action::*;
        use Phase::*;
        let cases = [
            (Prepared, Observed { current: false, staged: false, previous: false }, Fail),
            (Prepared, Observed { current: true,  staged: false, previous: false }, DiscardStaged),
            (Prepared, Observed { current: false, staged: true,  previous: false }, Fail),
            (Prepared, Observed { current: true,  staged: true,  previous: false }, ResumeSwap),
            (Prepared, Observed { current: false, staged: false, previous: true  }, RestorePrevious),
            (Prepared, Observed { current: true,  staged: false, previous: true  }, DiscardStaged),
            (Prepared, Observed { current: false, staged: true,  previous: true  }, RestorePrevious),
            (Prepared, Observed { current: true,  staged: true,  previous: true  }, ResumeSwap),
            (Copied, Observed { current: false, staged: false, previous: false }, Fail),
            (Copied, Observed { current: true,  staged: false, previous: false }, Fail),
            (Copied, Observed { current: false, staged: true,  previous: false }, Fail),
            (Copied, Observed { current: true,  staged: true,  previous: false }, ResumeSwap),
            (Copied, Observed { current: false, staged: false, previous: true  }, RestorePrevious),
            (Copied, Observed { current: true,  staged: false, previous: true  }, Fail),
            (Copied, Observed { current: false, staged: true,  previous: true  }, RestorePrevious),
            (Copied, Observed { current: true,  staged: true,  previous: true  }, ResumeSwap),
            (Swapped, Observed { current: false, staged: false, previous: false }, Fail),
            (Swapped, Observed { current: true,  staged: false, previous: false }, Fail),
            (Swapped, Observed { current: false, staged: true,  previous: false }, Fail),
            (Swapped, Observed { current: true,  staged: true,  previous: false }, SavePrevious),
            (Swapped, Observed { current: false, staged: false, previous: true  }, RestorePrevious),
            (Swapped, Observed { current: true,  staged: false, previous: true  }, RespawnCurrent),
            (Swapped, Observed { current: false, staged: true,  previous: true  }, RestorePrevious),
            (Swapped, Observed { current: true,  staged: true,  previous: true  }, SavePrevious),
            (PreviousSaved, Observed { current: false, staged: false, previous: false }, Fail),
            (PreviousSaved, Observed { current: true,  staged: false, previous: false }, Fail),
            (PreviousSaved, Observed { current: false, staged: true,  previous: false }, Fail),
            (PreviousSaved, Observed { current: true,  staged: true,  previous: false }, Fail),
            (PreviousSaved, Observed { current: false, staged: false, previous: true  }, RestorePrevious),
            (PreviousSaved, Observed { current: true,  staged: false, previous: true  }, RespawnCurrent),
            (PreviousSaved, Observed { current: false, staged: true,  previous: true  }, RestorePrevious),
            (PreviousSaved, Observed { current: true,  staged: true,  previous: true  }, RespawnCurrent),
            (Respawned, Observed { current: false, staged: false, previous: false }, Fail),
            (Respawned, Observed { current: true,  staged: false, previous: false }, Finish),
            (Respawned, Observed { current: false, staged: true,  previous: false }, Fail),
            (Respawned, Observed { current: true,  staged: true,  previous: false }, Finish),
            (Respawned, Observed { current: false, staged: false, previous: true  }, RestorePrevious),
            (Respawned, Observed { current: true,  staged: false, previous: true  }, Finish),
            (Respawned, Observed { current: false, staged: true,  previous: true  }, RestorePrevious),
            (Respawned, Observed { current: true,  staged: true,  previous: true  }, Finish),
            (Done, Observed { current: false, staged: false, previous: false }, Fail),
            (Done, Observed { current: true,  staged: false, previous: false }, Finish),
            (Done, Observed { current: false, staged: true,  previous: false }, Fail),
            (Done, Observed { current: true,  staged: true,  previous: false }, Finish),
            (Done, Observed { current: false, staged: false, previous: true  }, RestorePrevious),
            (Done, Observed { current: true,  staged: false, previous: true  }, Finish),
            (Done, Observed { current: false, staged: true,  previous: true  }, RestorePrevious),
            (Done, Observed { current: true,  staged: true,  previous: true  }, Finish),
            (RollbackPrepared, Observed { current: false, staged: false, previous: false }, Fail),
            (RollbackPrepared, Observed { current: true,  staged: false, previous: false }, Fail),
            (RollbackPrepared, Observed { current: false, staged: true,  previous: false }, Fail),
            (RollbackPrepared, Observed { current: true,  staged: true,  previous: false }, Fail),
            (RollbackPrepared, Observed { current: false, staged: false, previous: true  }, RestorePrevious),
            (RollbackPrepared, Observed { current: true,  staged: false, previous: true  }, RestorePrevious),
            (RollbackPrepared, Observed { current: false, staged: true,  previous: true  }, RestorePrevious),
            (RollbackPrepared, Observed { current: true,  staged: true,  previous: true  }, RestorePrevious),
            (RolledBack, Observed { current: false, staged: false, previous: false }, Fail),
            (RolledBack, Observed { current: true,  staged: false, previous: false }, Finish),
            (RolledBack, Observed { current: false, staged: true,  previous: false }, Fail),
            (RolledBack, Observed { current: true,  staged: true,  previous: false }, Finish),
            (RolledBack, Observed { current: false, staged: false, previous: true  }, RestorePrevious),
            (RolledBack, Observed { current: true,  staged: false, previous: true  }, Finish),
            (RolledBack, Observed { current: false, staged: true,  previous: true  }, RestorePrevious),
            (RolledBack, Observed { current: true,  staged: true,  previous: true  }, Finish),
        ];
        assert_eq!(cases.len(), 64);
        for (phase, observed, expected) in cases {
            assert_eq!(classify(phase, observed), expected,
                "wrong recovery action for {phase:?} with {observed:?}");
        }
    }

    #[test]
    fn missing_current_only_restores_previous_or_fails() {
        let phases = [Phase::Prepared, Phase::Copied, Phase::Swapped,
            Phase::PreviousSaved, Phase::Respawned, Phase::Done,
            Phase::RollbackPrepared, Phase::RolledBack];
        for phase in phases {
            for staged in [false, true] {
                for previous in [false, true] {
                    let observed = Observed { current: false, staged, previous };
                    let expected = if previous {
                        Action::RestorePrevious
                    } else {
                        Action::Fail
                    };
                    assert_eq!(classify(phase, observed), expected,
                        "unsafe recovery action for {phase:?} with {observed:?}");
                }
            }
        }
    }
}
