use autograd as ag;
use autograd::ndarray_ext as arr;

use crate::stats;

use ag::tensor::Variable;

pub fn price_call_option<'graph, F: ag::Float>(
    g: &'graph ag::Graph<F>,
    spot_price: &ag::Tensor<'graph, F>,
    time_to_maturity: &ag::Tensor<'graph, F>,
    strike_price: &ag::Tensor<'graph, F>,
    volatility: &ag::Tensor<'graph, F>,
    risk_free_interest_rate: F,
) -> ag::Tensor<'graph, F> {
    let zero = F::zero();
    let one = F::one();
    let two = F::from(2f64).unwrap();
    let d1 = g.ln(spot_price / strike_price)
        + time_to_maturity * ((g.pow(volatility, two) / two) + risk_free_interest_rate);
    let d2 = d1 - volatility * g.sqrt(time_to_maturity);

    spot_price * stats::normal::cdf(g, &d1, zero, one)
        - strike_price
            * g.exp(g.neg(time_to_maturity * risk_free_interest_rate))
            * stats::normal::cdf(g, &d2, zero, one)
}

pub fn price_put_option<'graph, F: ag::Float>(
    g: &'graph ag::Graph<F>,
    spot_price: &ag::Tensor<'graph, F>,
    time_to_maturity: &ag::Tensor<'graph, F>,
    strike_price: &ag::Tensor<'graph, F>,
    volatility: &ag::Tensor<'graph, F>,
    risk_free_interest_rate: F,
) -> ag::Tensor<'graph, F> {
    let zero = F::zero();
    let one = F::one();
    let two = F::from(2f64).unwrap();
    let d1 = g.ln(spot_price / strike_price)
        + time_to_maturity * ((g.pow(volatility, two) / two) + risk_free_interest_rate);
    let d2 = d1 - volatility * g.sqrt(time_to_maturity);

    strike_price
        * g.exp(g.neg(time_to_maturity * risk_free_interest_rate))
        * stats::normal::cdf(g, &g.neg(d2), zero, one)
        - spot_price * stats::normal::cdf(g, &g.neg(d1), zero, one)
}

pub fn implied_call_volatility<F: ag::Float>(
    given_call_price: &ag::NdArray<F>,
    given_spot_price: &ag::NdArray<F>,
    given_time_to_maturity: &ag::NdArray<F>,
    given_strike_price: &ag::NdArray<F>,
    risk_free_interest_rate: F,
    epochs: usize,
) -> ag::NdArray<F> {
    assert!(given_call_price
        .shape()
        .iter()
        .zip(
            given_spot_price.shape().iter().zip(
                given_time_to_maturity
                    .shape()
                    .iter()
                    .zip(given_strike_price.shape().iter())
            )
        )
        .all(|(a, (b, (c, d)))| { *a == *b && *b == *c && *c == *d }));
    let rng = arr::ArrayRng::<F>::default();
    let volatility_arr = arr::into_shared(rng.standard_uniform(given_call_price.shape()));
    let adam_state = ag::optimizers::adam::AdamState::new(&[&volatility_arr]);

    for _ in 0..epochs {
        ag::with(|g: &mut ag::Graph<F>| {
            let volatility = g.variable(volatility_arr.clone());
            let call_price = g.placeholder(&[-1]);
            let spot_price = g.placeholder(&[-1]);
            let time_to_maturity = g.placeholder(&[-1]);
            let strike_price = g.placeholder(&[-1]);

            let predicted_call_price = price_call_option(
                g,
                &spot_price,
                &time_to_maturity,
                &strike_price,
                &volatility,
                risk_free_interest_rate,
            );
            let mean_loss = g.reduce_mean(g.square(predicted_call_price - call_price), &[-1], false);
            let grads = &g.grad(&[mean_loss], &[volatility]);
            let update_ops = &ag::optimizers::adam::Adam::default().compute_updates(
                &[volatility],
                grads,
                &adam_state,
                g,
            );

            g.eval(
                update_ops,
                &[
                    call_price.given(given_call_price.view().into_dyn()),
                    spot_price.given(given_spot_price.view().into_dyn()),
                    time_to_maturity.given(given_time_to_maturity.view().into_dyn()),
                    strike_price.given(given_strike_price.view().into_dyn()),
                ],
            );
        });
    }
    let locked = volatility_arr
        .read()
        .expect("Could not read lock the volatility array"); 
    locked.to_owned()
}

pub fn implied_put_volatility<F: ag::Float>(
    given_put_price: &ag::NdArray<F>,
    given_spot_price: &ag::NdArray<F>,
    given_time_to_maturity: &ag::NdArray<F>,
    given_strike_price: &ag::NdArray<F>,
    risk_free_interest_rate: F,
    epochs: usize,
) -> ag::NdArray<F> {
    assert!(given_put_price
        .shape()
        .iter()
        .zip(
            given_spot_price.shape().iter().zip(
                given_time_to_maturity
                    .shape()
                    .iter()
                    .zip(given_strike_price.shape().iter())
            )
        )
        .all(|(a, (b, (c, d)))| { *a == *b && *b == *c && *c == *d }));
    let rng = arr::ArrayRng::<F>::default();
    let volatility_arr = arr::into_shared(rng.standard_uniform(given_put_price.shape()));
    let adam_state = ag::optimizers::adam::AdamState::new(&[&volatility_arr]);

    for _ in 0..epochs {
        ag::with(|g: &mut ag::Graph<F>| {
            let volatility = g.variable(volatility_arr.clone());
            let put_price = g.placeholder(&[-1]);
            let spot_price = g.placeholder(&[-1]);
            let time_to_maturity = g.placeholder(&[-1]);
            let strike_price = g.placeholder(&[-1]);

            let predicted_put_price = price_put_option(
                g,
                &spot_price,
                &time_to_maturity,
                &strike_price,
                &volatility,
                risk_free_interest_rate,
            );
            let mean_loss = g.reduce_mean(g.square(predicted_put_price - put_price), &[-1], false);
            let grads = &g.grad(&[mean_loss], &[volatility]);
            let update_ops = &ag::optimizers::adam::Adam::default().compute_updates(
                &[volatility],
                grads,
                &adam_state,
                g,
            );

            g.eval(
                update_ops,
                &[
                    put_price.given(given_put_price.view().into_dyn()),
                    spot_price.given(given_spot_price.view().into_dyn()),
                    time_to_maturity.given(given_time_to_maturity.view().into_dyn()),
                    strike_price.given(given_strike_price.view().into_dyn()),
                ],
            );
        });
    }
    let locked = volatility_arr
        .read()
        .expect("Could not read lock the volatility array"); 
    locked.to_owned()
}
