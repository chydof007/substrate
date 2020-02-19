// Copyright 2020 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Tools for analysing the benchmark results.

use linregress::{FormulaRegressionBuilder, RegressionDataBuilder, RegressionModel};
use frame_benchmarking::{BenchmarkResults, BenchmarkParameter};
use std::collections::BTreeMap;

pub struct Analysis {
	base: u128,
	parameters: Vec<(BenchmarkParameter, u128)>,
	value_dists: Vec<(Vec<u32>, u128, u128)>,
	params: Vec<String>,
	model: RegressionModel,
}

impl Analysis {
	pub fn from_results(r: &Vec<BenchmarkResults>) -> Option<Self> {
		let mut results = BTreeMap::<Vec<u32>, Vec<u128>>::new();
		for &(ref params, t) in r.iter() {
			let p = params.iter().map(|x| x.1).collect::<Vec<_>>();
			results.entry(p).or_default().push(t);
		}
		for (_, rs) in results.iter_mut() {
			rs.sort();
			let ql = rs.len() / 4;
			*rs = rs[ql..rs.len() - ql].to_vec();
		}

		let mut data = vec![("Y", results.iter().flat_map(|x| x.1.iter().map(|v| *v as f64)).collect())];

		let params = r[0].0.iter().map(|x| format!("{:?}", x.0)).collect::<Vec<_>>();
		data.extend(params.iter()
			.enumerate()
			.map(|(i, p)| (
				p.as_str(),
				results.iter()
					.flat_map(|x| Some(x.0[i] as f64)
						.into_iter()
						.cycle()
						.take(x.1.len())
					).collect::<Vec<_>>()
			))
		);

//		println!("data: {:?}", data);
		let data = RegressionDataBuilder::new().build_from(data).ok()?;

		let formula = format!("Y ~ {}", params.join(" + "));
//		println!("formula: {:?}", formula);
		let model = FormulaRegressionBuilder::new()
			.data(&data)
			.formula(formula)
			.fit()
			.ok()?;

		let parameters = model.parameters.regressor_values.iter()
			.enumerate()
			.map(|(i, x)| (r[0].0[i].0, (*x + 0.5) as u128))
			.collect();

		let value_dists = results.iter().map(|(p, vs)| {
			let total = vs.iter()
				.fold(0u128, |acc, v| acc + *v);
			let mean = total / vs.len() as u128;
			let sum_sq_diff = vs.iter()
				.fold(0u128, |acc, v| {
					let d = mean.max(*v) - mean.min(*v);
					acc + d * d
				});
			let stddev = (sum_sq_diff as f64 / vs.len() as f64).sqrt() as u128;
			(p.clone(), mean, stddev)
		}).collect::<Vec<_>>();

		Some(Self {
			base: (model.parameters.intercept_value + 0.5) as u128,
			parameters,
			value_dists,
			params,
			model,
		})
	}
}

fn ms(mut nanos: u128) -> String {
	let mut x = 100_000u128;
	while x > 1 {
		if nanos > x * 1_000 {
			nanos = nanos / x * x;
			break;
		}
		x /= 10;
	}
	format!("{}", nanos as f64 / 1_000f64)
}

impl std::fmt::Display for Analysis {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		writeln!(f, "Data points distribution:")?;
		writeln!(f, "{}   mean µs  sigma µs       %", self.params.iter().map(|p| format!("{:>5}", p)).collect::<Vec<_>>().join(" "))?;
		for (param_values, mean, sigma) in self.value_dists.iter() {
			writeln!(f, "{}  {:>8}  {:>8}  {:>3}.{}%",
				param_values.iter().map(|v| format!("{:>5}", v)).collect::<Vec<_>>().join(" "),
				ms(*mean),
				ms(*sigma),
				(sigma * 100 / mean),
				(sigma * 1000 / mean % 10)
			)?;
		}

		writeln!(f, "\nQuality and confidence:")?;
		writeln!(f, "param     error", p, ms(*se as u128))?;
		for (p, se) in self.params.iter().zip(self.model.se.regressor_values.iter()) {
			writeln!(f, "{}      {:>8}", p, ms(*se as u128))?;
		}

		writeln!(f, "\nModel:")?;
		writeln!(f, "Time ~= {:>8}", ms(self.base))?;
		for &(p, t) in self.parameters.iter() {
			writeln!(f, "    + {:?} {:>8}", p, ms(t))?;
		}
		writeln!(f, "              µs")
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn analysis_should_work() {
		let a = Analysis::from_results(&vec![
			(vec![(BenchmarkParameter::N, 1), (BenchmarkParameter::M, 5)], 11_500_000),
			(vec![(BenchmarkParameter::N, 2), (BenchmarkParameter::M, 5)], 12_500_000),
			(vec![(BenchmarkParameter::N, 3), (BenchmarkParameter::M, 5)], 13_500_000),
			(vec![(BenchmarkParameter::N, 4), (BenchmarkParameter::M, 5)], 14_500_000),
			(vec![(BenchmarkParameter::N, 3), (BenchmarkParameter::M, 1)], 13_100_000),
			(vec![(BenchmarkParameter::N, 3), (BenchmarkParameter::M, 3)], 13_300_000),
			(vec![(BenchmarkParameter::N, 3), (BenchmarkParameter::M, 7)], 13_700_000),
			(vec![(BenchmarkParameter::N, 3), (BenchmarkParameter::M, 10)], 14_000_000),
		]).unwrap();
		assert_eq!(a.base, 10_000_000);
		assert_eq!(a.parameters, vec![
			(BenchmarkParameter::N, 1_000_000),
			(BenchmarkParameter::M, 100_000)
		]);
	}
}