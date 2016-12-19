use std::cell::Cell;
use std::fmt::{self, Debug, Formatter};
use std::io;

use rand::{Rng, StdRng};

pub struct Distribution {
    limit: u32,
    // TODO: Figure out how to get rid of interior mutability
    rng: Cell<StdRng>,
    // TODO: Decide if there should be a limit to the size of the table, so we don't use a massive amount of memory on large limits
    cumulative_probability_table: Vec<f64>
}

impl Distribution {
    pub fn new(density_function: &ProbabilityDensityFunction, limit: u32) -> io::Result<Distribution> {
        let rng = StdRng::new()?;

        let mut lookup_table: Vec<f64> = Vec::with_capacity(limit as usize);
        lookup_table.push(0.0);

        let mut cumulative_probability = 0.0;
        for i  in 1..(limit + 1) {
            cumulative_probability += density_function.density(i, limit);
            lookup_table.push(cumulative_probability);
        }

        Ok(Distribution {
            limit: limit,
            rng: Cell::new(rng),
            cumulative_probability_table: lookup_table
        })
    }

    pub fn query(&self) -> u32 {
        let selector = self.query_interior_rng_float();

        for i in 1..(self.limit + 1) {
            if selector < self.cumulative_probability_table[i as usize] {
                return i;
            }
        }

        panic!("Cumulative probabilities don't sum to 1! (limit is {}, probability table is {:?})", self.limit, self.cumulative_probability_table)
    }

    // TODO: Exposing this method is an ugly hack that should be removed
    pub fn query_interior_rng_usize(&self, start: usize, end: usize) -> usize {
        let mut rng = self.rng.get();
        let result = rng.gen_range(start, end);
        self.rng.set(rng);

        result
    }

    fn query_interior_rng_float(&self) -> f64 {
        let mut rng = self.rng.get();
        let result = rng.next_f64();
        self.rng.set(rng);

        result
    }
}

impl Debug for Distribution {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        fmt.debug_struct("Distribution")
            .field("limit", &self.limit)
            .field("rng", &"StdRng")
            .field("cumulative_probability_table", &self.cumulative_probability_table)
            .finish()
    }
}

// Define various ProbabilityDensityFunctions
pub trait ProbabilityDensityFunction {
    fn density(&self, point: u32, limit: u32) -> f64;
}

pub struct IdealSolitonDistribution;

impl ProbabilityDensityFunction for IdealSolitonDistribution {
    fn density(&self, point: u32, limit: u32) -> f64 {
        if point == 0 || point > limit {
            panic!("Point must be in the range (0, limit], but was really {}! (the limit was {})", point, limit);
        }else if point == 1 {
            1.0 / (limit as f64)
        } else {
            1.0 / ((point as f64) * (point as f64 - 1.0))
        }
    }
}

pub struct RobustSolitonDistribution {
    failure_probability: f64,
    expected_ripple_size: ExpectedRippleSize
}

impl RobustSolitonDistribution {
    // TODO: Remove this allow
    #[allow(dead_code)]
    pub fn new(failure_probability: f64, expected_ripple_size: f64) -> RobustSolitonDistribution {
        RobustSolitonDistribution {
            failure_probability: failure_probability,
            expected_ripple_size: ExpectedRippleSize::Exactly(expected_ripple_size)
        }
    }

    pub fn new_using_heuristic(failure_probability: f64, hint_constant: f64) -> RobustSolitonDistribution {
        RobustSolitonDistribution {
            failure_probability: failure_probability,
            expected_ripple_size: ExpectedRippleSize::BasedOnHeuristic(hint_constant)
        }
    }

    // Helper methods for the density calculation
    fn normalization_factor(&self, limit: u32) -> f64{
        let mut normalization_factor = 0.0;
        for i in 1..(limit + 1) {
            normalization_factor += IdealSolitonDistribution.density(i, limit);
            normalization_factor += self.robustness_probability_to_add(i, limit);
        }
        normalization_factor
    }

    fn robustness_probability_to_add(&self, point: u32, limit: u32) -> f64{
        let failure_probability = self.failure_probability;
        let expected_ripple_size = self.expected_ripple_size.get(limit, self.failure_probability);

        let switch_point = (limit as f64 / expected_ripple_size) as u32;

        if point == 0 || point > limit {
            panic!("Point must be in the range (0, limit], but was really {}! (the limit was {})", point, limit);
        }else if point < switch_point {
            expected_ripple_size / ((point * limit) as f64)
        }else if point == switch_point {
            (expected_ripple_size * (expected_ripple_size / failure_probability).ln()) / (limit as f64)
        }else {
            0.0
        }
    }
}

impl ProbabilityDensityFunction for RobustSolitonDistribution {
    fn density(&self, point: u32, limit: u32) -> f64 {
        if point == 0 || point > limit {
            panic!("Point must be in the range (0, limit], but was really {}! (the limit was {})", point, limit);
        }
        // Special case this to prevent normally good values of expected_ripple_size from failing
        if limit == 1 {
            1.0
        } else {
            (IdealSolitonDistribution.density(point, limit) +
                self.robustness_probability_to_add(point, limit)
            ) / self.normalization_factor(limit)
        }
    }
}

enum ExpectedRippleSize {
    // TODO: Remove this allow
    #[allow(dead_code)]
    Exactly(f64),
    BasedOnHeuristic(f64)
}

impl ExpectedRippleSize {
    fn get(&self, limit: u32, failure_probability: f64) -> f64 {
        match self {
            &ExpectedRippleSize::Exactly(val) => {
                val
            }
            // TODO: Figure out if the hint_constant can sensibly be bigger than 1
            &ExpectedRippleSize::BasedOnHeuristic(hint_constant) => {
                hint_constant * (limit as f64 / failure_probability).ln() * (limit as f64).sqrt()
            }
        }
    }
}


// TODO: Replace the distribution tests
//#[cfg(test)]
//mod test {
//    use super::{ideal_soliton_probability_density, robust_soliton_probability_density, expected_ripple_size_heuristic};
//
//    #[test]
//    fn check_ideal_soliton_for_small_values() {
//        assert_eq!(ideal_soliton_probability_density(1, 10), 0.1);
//
//        assert_eq!(ideal_soliton_probability_density(2, 10), 0.5);
//
//        assert_eq!(ideal_soliton_probability_density(3, 10), 1.0/6.0);
//    }
//
//    #[test]
//    fn robust_soliton_sanity_test() {
//        let limit = 100;
//        let failure_probability = 0.1;
//        let hint_constant = 0.1;
//
//        let expected_ripple_size = expected_ripple_size_heuristic(limit, failure_probability, hint_constant);
//
//        let mut cumulative_probability = 0.0;
//        for i in 1..20 {
//            cumulative_probability += robust_soliton_probability_density(i, limit, failure_probability, expected_ripple_size);
//        }
//        println!("Cumulative probability is {}", cumulative_probability);
//        assert!(cumulative_probability > 0.9);
//    }
//}