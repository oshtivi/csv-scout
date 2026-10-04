// states: 0 = SteadyStrict, 1 = SteadyFlexible, 2 = Unsteady
pub const N_STATES: usize = 3;
pub const STATE_STEADYSTRICT: usize = 0;
pub const STATE_STEADYFLEX: usize = 1;
pub const STATE_UNSTEADY: usize = 2;
// observations: 0 = MaxValue, 1 = Other, 2 = Zero
pub const N_OBS: usize = 3;
pub const OBS_MAXVALUE: usize = 0;
pub const OBS_OTHER: usize = 1;
pub const OBS_ZERO: usize = 2;

#[derive(Debug, Clone, Copy)]
pub struct VIteration {
    pub(crate) prob: f64,
    pub(crate) prev: Option<usize>,
}
#[derive(Debug, Clone)]
pub struct ViterbiResults {
    pub(crate) max_delim_freq: usize,
    pub(crate) path: Vec<(usize, VIteration)>,
}

/// Result of the numerically-stable (scaled) Viterbi search.
#[derive(Debug, Clone, Copy)]
pub struct ScaledViterbiResults {
    pub(crate) max_delim_freq: usize,
    /// Most likely final state, or `None` if there were no observations.
    pub(crate) final_state: Option<usize>,
    /// Natural log of the probability of the most likely path (`-inf` if impossible).
    pub(crate) log_prob: f64,
}

#[derive(Debug, Default, Clone)]
pub struct Chain {
    observations: Vec<usize>,
}
impl Chain {
    pub(crate) fn add_observation(&mut self, obs: usize) {
        self.observations.push(obs);
    }
    pub(crate) fn viterbi(&mut self) -> ViterbiResults {
        if self.observations.is_empty() {
            return ViterbiResults {
                max_delim_freq: 0,
                path: vec![],
            };
        }
        // compute the max frequency value; unwrap is safe, we just checked if vector is empty
        let max_value = *self.observations.iter().max().unwrap();
        if max_value == 0 {
            // no frequencies observed! return unsteady state
            return ViterbiResults {
                max_delim_freq: max_value,
                path: vec![(
                    STATE_UNSTEADY,
                    VIteration {
                        prob: 0.0,
                        prev: Some(STATE_UNSTEADY),
                    },
                )],
            };
        }

        let start_prob = [
            1.0 / 3.0, /*SteadyStrict*/
            1.0 / 3.0, /*SteadyFlexible*/
            1.0 / 3.0, /*Unsteady*/
        ];
        let mut trans_prob = [
            /*FromSteadyStrict*/
            1.0, 0.0, 0.0, /* ToSteadyStrict, ToSteadyFlexible, ToUnsteady */
            /*FromSteadyFlexible*/
            0.0, 1.0, 0.0, /* ToSteadyStrict, ToSteadyFlexible, ToUnsteady */
            /*FromUnsteady*/
            0.2, 0.2, 0.6, /* ToSteadyStrict, ToSteadyFlexible, ToUnsteady */
        ];
        let update_trans_prob = decay_trans_prob;

        let emit_uniprob = 1.0 / (max_value as f64 + 1.0);
        let emit_prob = [
            /*FromSteadyStrict*/
            1.0, /* MaxValue */
            0.0, /* Other */
            0.0, /* Zero */
            /*FromSteadyFlexible*/
            0.7, /* MaxValue */
            0.3, /* Other */
            0.0, /* Zero */
            /*FromUnsteady*/
            emit_uniprob, /* MaxValue */
            // 1.0 - 2.0 * emit_uniprob, /* Other */
            // below is the fused multiply add version
            2.0f64.mul_add(-emit_uniprob, 1.0),
            emit_uniprob, /* Zero */
        ];
        // function to map frequency to observation
        let map_observation = |freq: usize| {
            if freq == max_value {
                OBS_MAXVALUE
            } else if freq == 0 {
                OBS_ZERO
            } else {
                OBS_OTHER
            }
        };

        let mut iterations: Vec<Vec<VIteration>> = vec![vec![]];
        for prob_val in start_prob.iter().take(N_STATES) {
            iterations[0].push(VIteration {
                prob: *prob_val,
                prev: None,
            });
        }

        for t in 0..self.observations.len() {
            // since we start with iterations already at length 1, the index of this newly-pushed
            // vector will be t + 1.
            iterations.push(vec![]);
            for state_idx in 0..N_STATES {
                let (max_prev_st, max_tr_prob) =
                    (0..N_STATES).fold((None, 0.0), |acc, prev_state_idx| {
                        let tr_prob = iterations[t][prev_state_idx].prob
                            * trans_prob[prev_state_idx * N_STATES + state_idx];
                        if acc.0.is_none() || tr_prob > acc.1 {
                            (Some(prev_state_idx), tr_prob)
                        } else {
                            acc
                        }
                    });
                assert!(
                    max_prev_st.is_some(),
                    "All previous states at 0.0 probability"
                );
                iterations[t + 1].push(VIteration {
                    prob: max_tr_prob
                        * emit_prob[state_idx * N_OBS + map_observation(self.observations[t])],
                    prev: max_prev_st,
                });
                update_trans_prob(&mut trans_prob);
            }
        }

        let (final_state, final_viter) = iterations[iterations.len() - 1].iter().enumerate().fold(
            (0, None),
            |acc: (usize, Option<VIteration>), (state, &viter)| match acc.1 {
                Some(max_viter) => {
                    if viter.prob > max_viter.prob {
                        (state, Some(viter))
                    } else {
                        acc
                    }
                }
                None => (state, Some(viter)),
            },
        );
        let final_viter = final_viter.expect("All final states at 0.0 probability");
        let mut path = vec![(final_state, final_viter)];
        for t in (-1isize..iterations.len() as isize - 2).rev() {
            let prev_viter = path[path.len() - 1].1;
            let prev_state = prev_viter
                .prev
                .expect("all iterations should have a previous state except initial iteration");
            path.push((prev_state, iterations[(t + 1) as usize][prev_state]));
        }
        path.reverse();
        ViterbiResults {
            max_delim_freq: max_value,
            path,
        }
    }

    /// Log-space equivalent of [`viterbi`](Self::viterbi) that only tracks the final state.
    ///
    /// The model (start, transition, emission probabilities and the per-step transition decay) is
    /// identical, so for inputs where `viterbi` does not underflow both pick the same final state.
    /// Unlike `viterbi`, path probabilities here do not underflow to `0.0` on long samples (which
    /// would otherwise make every chain look `SteadyStrict`).
    pub(crate) fn scaled_viterbi(&self) -> ScaledViterbiResults {
        let Some(&max_value) = self.observations.iter().max() else {
            return ScaledViterbiResults {
                max_delim_freq: 0,
                final_state: None,
                log_prob: f64::NEG_INFINITY,
            };
        };
        if max_value == 0 {
            return ScaledViterbiResults {
                max_delim_freq: 0,
                final_state: Some(STATE_UNSTEADY),
                log_prob: f64::NEG_INFINITY,
            };
        }

        let mut trans_prob: [f64; N_STATES * N_STATES] = [
            1.0, 0.0, 0.0, /* from SteadyStrict */
            0.0, 1.0, 0.0, /* from SteadyFlexible */
            0.2, 0.2, 0.6, /* from Unsteady */
        ];
        let emit_uniprob = 1.0 / (max_value as f64 + 1.0);
        let emit_prob = [
            1.0,
            0.0,
            0.0,
            0.7,
            0.3,
            0.0,
            emit_uniprob,
            2.0f64.mul_add(-emit_uniprob, 1.0),
            emit_uniprob,
        ];
        let map_observation = |freq: usize| {
            if freq == max_value {
                OBS_MAXVALUE
            } else if freq == 0 {
                OBS_ZERO
            } else {
                OBS_OTHER
            }
        };

        let mut current = [(1.0f64 / 3.0).ln(); N_STATES];
        for &obs in &self.observations {
            let obs = map_observation(obs);
            let mut next = [f64::NEG_INFINITY; N_STATES];
            for (state_idx, next_val) in next.iter_mut().enumerate() {
                // same argmax (first max wins) as `viterbi`, but on log-probabilities
                let mut best: Option<f64> = None;
                for (prev_state_idx, &prev) in current.iter().enumerate() {
                    let tr = prev + trans_prob[prev_state_idx * N_STATES + state_idx].ln();
                    if best.is_none() || best.is_some_and(|b| tr > b) {
                        best = Some(tr);
                    }
                }
                *next_val =
                    best.unwrap_or(f64::NEG_INFINITY) + emit_prob[state_idx * N_OBS + obs].ln();
                // `viterbi` decays the Unsteady->Steady transitions once per state per step;
                // mirror that exactly.
                decay_trans_prob(&mut trans_prob);
            }
            current = next;
        }

        // first max wins, matching `viterbi`
        let (final_state, log_prob) =
            current
                .iter()
                .enumerate()
                .fold(
                    (0, current[0]),
                    |acc, (state, &p)| if p > acc.1 { (state, p) } else { acc },
                );
        ScaledViterbiResults {
            max_delim_freq: max_value,
            final_state: Some(final_state),
            log_prob,
        }
    }
}

// Decrements the Unsteady->Steady transitions by DELTA and increments Unsteady->Unsteady by
// 2*DELTA, making it progressively harder to leave the Unsteady state.
fn decay_trans_prob(trans_prob: &mut [f64; N_STATES * N_STATES]) {
    const DELTA: f64 = 0.01;
    trans_prob[STATE_UNSTEADY * N_STATES + STATE_STEADYSTRICT] =
        (trans_prob[STATE_UNSTEADY * N_STATES + STATE_STEADYSTRICT] - DELTA).max(0.0);
    trans_prob[STATE_UNSTEADY * N_STATES + STATE_STEADYFLEX] =
        (trans_prob[STATE_UNSTEADY * N_STATES + STATE_STEADYFLEX] - DELTA).max(0.0);
    trans_prob[STATE_UNSTEADY * N_STATES + STATE_UNSTEADY] = 2.0f64
        .mul_add(
            DELTA,
            trans_prob[STATE_UNSTEADY * N_STATES + STATE_UNSTEADY],
        )
        .min(1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(obs: impl IntoIterator<Item = usize>) -> Chain {
        let mut c = Chain::default();
        obs.into_iter().for_each(|o| c.add_observation(o));
        c
    }

    fn final_state_of_viterbi(c: &mut Chain) -> usize {
        let r = c.viterbi();
        r.path[r.path.len() - 1].0
    }

    #[test]
    fn scaled_matches_viterbi_on_short_inputs() {
        let cases: Vec<Vec<usize>> = vec![
            vec![3; 50],
            (0..50).map(|i| if i % 5 == 0 { 2 } else { 3 }).collect(),
            (0..50).map(|i| i % 4).collect(),
            vec![0, 1, 0, 0, 2, 0, 5, 5, 5, 5, 5],
            vec![1, 3, 3, 3, 3, 3, 3, 3],
            vec![2],
        ];
        for obs in cases {
            let mut c = chain(obs.clone());
            let scaled = c.scaled_viterbi();
            assert_eq!(
                scaled.final_state,
                Some(final_state_of_viterbi(&mut c)),
                "{obs:?}"
            );
        }
    }

    #[test]
    fn scaled_does_not_underflow_on_long_inputs() {
        // the plain viterbi underflows to prob 0.0 here and reports state 0 (SteadyStrict)
        let c = chain((0..5000).map(|i| i % 4));
        let r = c.scaled_viterbi();
        assert_eq!(r.final_state, Some(STATE_UNSTEADY));
        assert!(r.log_prob.is_finite());

        let c = chain((0..5000).map(|i| if i % 5 == 0 { 2 } else { 3 }));
        assert_eq!(c.scaled_viterbi().final_state, Some(STATE_STEADYFLEX));

        let c = chain(std::iter::repeat_n(7, 5000));
        assert_eq!(c.scaled_viterbi().final_state, Some(STATE_STEADYSTRICT));
    }

    #[test]
    fn scaled_empty_and_zero() {
        assert_eq!(Chain::default().scaled_viterbi().final_state, None);
        assert_eq!(
            chain([0, 0, 0]).scaled_viterbi().final_state,
            Some(STATE_UNSTEADY)
        );
    }
}
