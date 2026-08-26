#!/usr/bin/env python3
"""Designs Velour's oversampling halfband filters.

Writes `plugins/velour/velour-core/src/oversample/coefficients.rs`. Run it with

    mise run oversampler:design

**Nothing at build or run time needs this.** The coefficients are checked in;
this script exists so the numbers can be re-derived and so the reasoning behind
them is executable rather than a comment.

The structure is a polyphase IIR halfband (Valenzuela & Constantinides):

    H(z) = 1/2 * [ A0(z^2) + z^-1 * A1(z^2) ]

where each `A` is a cascade of first-order allpass sections
`(a + z^-2) / (1 + a * z^-2)`. Sorted ascending, the coefficients alternate
between the two branches. Every section is allpass, so the passband is flat by
construction and the design has exactly one thing to minimise: the worst
magnitude in the stopband.

**The stopband edges are not "Nyquist".** What matters is only the content that
folds back into the audible band, and working that out is what makes these
filters cheap enough to be free (see `STAGES` below).

Only the standard library: `cmath` for the response and a small Nelder-Mead for
the search. scipy would design these in one call, and adding a dependency to
re-derive fourteen numbers that never change is not a trade worth making.
"""

from __future__ import annotations

import cmath
import math
import random
import struct
from pathlib import Path

OUTPUT = Path("plugins/velour/velour-core/src/oversample/coefficients.rs")

# Everything below is worked out for a 48 kHz host running at 4x (192 kHz
# internally). The coefficients are in normalized frequency, so a host at
# another rate moves the real frequencies with it — which is the correct
# behaviour, because what has to be stopped is whatever folds past *that*
# host's Nyquist.
#
# Stage one, the 48 kHz <-> 96 kHz halfband. Decimating 96 -> 48 folds content
# above 24 kHz down. Content in 24..28 kHz lands in 20..24 kHz, which nobody
# hears; content above 28 kHz lands under 20 kHz, which everybody does. So the
# stopband starts at 28 kHz -> 2 * 28/96 = 0.583 of pi. Halfband symmetry then
# puts the passband edge at 0.417 of pi, which is 20 kHz. It works out exactly
# because 20 and 28 straddle 24.
#
# Stage two, the 96 kHz <-> 192 kHz halfband. Decimating 192 -> 96 folds content
# above 48 kHz. Anything landing above 28 kHz is stage one's problem, so this
# stage only has to stop what lands below it: 96 - f < 28 kHz, meaning f above
# 68 kHz -> 2 * 68/192 = 0.708 of pi. A much wider transition, and much cheaper.
#
# `sections` is chosen as the smallest count that clears 70 dB *without* a
# coefficient close to 1. A coefficient of 0.99 puts a pole at |z| = 0.995,
# which is more ringing and less f32 headroom than a spare allpass section
# costs. Stage one hits 75 dB with six sections and a 0.990 coefficient; seven
# sections reach 78 dB with nothing above 0.88.
STAGES = (
    ("STAGE_ONE", "48 kHz <-> 96 kHz", 0.5833, 7, 48),
    ("STAGE_TWO", "96 kHz <-> 192 kHz", 0.7083, 3, 40),
)

TARGET_DB = 70.0

# Fixed so the script is reproducible. The Rust side measures the attenuation
# the checked-in numbers actually deliver, so this seed is a convenience rather
# than a guarantee.
SEED = 11


def make_objective(edge: float, points: int):
    """The worst |H| across the stopband, as a function of the coefficients."""
    low = math.pi * edge
    grid = [low + (math.pi - low) * index / (points - 1) for index in range(points)]
    squared = [cmath.exp(-2j * w) for w in grid]
    single = [cmath.exp(-1j * w) for w in grid]

    def worst(coefficients: list[float]) -> float:
        even, odd = coefficients[0::2], coefficients[1::2]
        peak = 0.0
        for z2, z1 in zip(squared, single):
            branch0 = 1 + 0j
            for a in even:
                branch0 *= (a + z2) / (1 + a * z2)
            branch1 = 1 + 0j
            for a in odd:
                branch1 *= (a + z2) / (1 + a * z2)
            peak = max(peak, abs(branch0 + z1 * branch1))
        return 0.5 * peak

    return worst


def passband_ripple(coefficients: list[float], edge: float, points: int = 800) -> float:
    """Peak-to-peak passband ripple in dB. Should be nothing at all: a polyphase
    halfband is power-complementary, so a stopband at -78 dB leaves a ripple
    around 1e-8 dB. Reported to catch a structural mistake, not to be tuned."""
    high = math.pi * (1.0 - edge)
    magnitudes = []
    for index in range(points):
        w = high * index / (points - 1)
        z2, z1 = cmath.exp(-2j * w), cmath.exp(-1j * w)
        branch0 = 1 + 0j
        for a in coefficients[0::2]:
            branch0 *= (a + z2) / (1 + a * z2)
        branch1 = 1 + 0j
        for a in coefficients[1::2]:
            branch1 *= (a + z2) / (1 + a * z2)
        magnitudes.append(abs(0.5 * (branch0 + z1 * branch1)))
    return 20.0 * math.log10(max(magnitudes) / min(magnitudes))


def to_coefficient(t: float) -> float:
    """A logistic, so the search runs unconstrained and the coefficients stay in
    `(0, 1)` — outside that an allpass section is unstable."""
    return 1.0 / (1.0 + math.exp(-t))


def to_parameter(a: float) -> float:
    return math.log(a / (1.0 - a))


def nelder_mead(f, start: list[float], steps: int) -> tuple[list[float], float]:
    n = len(start)
    simplex = [list(start)]
    for index in range(n):
        point = list(start)
        point[index] += 0.5
        simplex.append(point)
    values = [f(point) for point in simplex]

    for _ in range(steps):
        order = sorted(range(n + 1), key=lambda i: values[i])
        simplex = [simplex[i] for i in order]
        values = [values[i] for i in order]

        centroid = [sum(p[i] for p in simplex[:-1]) / n for i in range(n)]
        worst = simplex[-1]

        reflected = [centroid[i] + (centroid[i] - worst[i]) for i in range(n)]
        reflected_value = f(reflected)

        if reflected_value < values[0]:
            expanded = [centroid[i] + 2.0 * (centroid[i] - worst[i]) for i in range(n)]
            expanded_value = f(expanded)
            if expanded_value < reflected_value:
                simplex[-1], values[-1] = expanded, expanded_value
            else:
                simplex[-1], values[-1] = reflected, reflected_value
        elif reflected_value < values[-2]:
            simplex[-1], values[-1] = reflected, reflected_value
        else:
            contracted = [centroid[i] + 0.5 * (worst[i] - centroid[i]) for i in range(n)]
            contracted_value = f(contracted)
            if contracted_value < values[-1]:
                simplex[-1], values[-1] = contracted, contracted_value
            else:
                for i in range(1, n + 1):
                    simplex[i] = [
                        (simplex[i][j] + simplex[0][j]) * 0.5 for j in range(n)
                    ]
                    values[i] = f(simplex[i])

    return simplex[0], values[0]


def design(sections: int, edge: float, restarts: int) -> tuple[list[float], float, float]:
    """Multi-start, because this objective has local minima that a single run
    settles into several dB short — six sections came out *worse* than four
    until the restarts went in."""
    rng = random.Random(SEED)
    coarse = make_objective(edge, 64)

    def objective(parameters: list[float]) -> float:
        return coarse(sorted(to_coefficient(v) for v in parameters))

    best, best_value = None, math.inf
    for attempt in range(restarts):
        if attempt == 0:
            # Spread toward the low end, which is roughly where an optimal
            # halfband's coefficients sit.
            guess = [
                0.03 + 0.94 * (index / (sections - 1) if sections > 1 else 0.5) ** 1.4
                for index in range(sections)
            ]
        else:
            guess = sorted(rng.uniform(0.01, 0.99) for _ in range(sections))

        point, value = nelder_mead(objective, [to_parameter(x) for x in guess], steps=900)
        if value < best_value:
            best, best_value = point, value

    coefficients = sorted(to_coefficient(v) for v in best)
    fine = make_objective(edge, 1200)
    attenuation = -20.0 * math.log10(fine(coefficients))
    return coefficients, attenuation, passband_ripple(coefficients, edge)


def as_f32_literal(value: float) -> str:
    """The shortest decimal that round-trips to the same `f32`.

    Not cosmetic: a literal carrying more digits than an `f32` can hold is a
    clippy error (`excessive_precision`), and the extra digits are a lie about
    the precision anyway.
    """
    target = struct.unpack("f", struct.pack("f", value))[0]
    for digits in range(1, 12):
        # Formatted from the f32 value, not from the double it came from: the
        # two can round to neighbouring f32s and then no short string matches.
        candidate = f"{target:.{digits}}"
        if struct.unpack("f", struct.pack("f", float(candidate)))[0] == target:
            return candidate
    return repr(target)


def main() -> None:
    lines = [
        "//! Halfband coefficients for the oversampler. **Generated.**",
        "//!",
        "//! Regenerate with `mise run oversampler:design`. The reasoning behind the",
        "//! stopband edges and the section counts is in",
        "//! `scripts/design-oversampler.py`; what the numbers actually achieve is",
        "//! measured by the tests in `super`.",
        "",
    ]

    for name, span, edge, sections, restarts in STAGES:
        coefficients, attenuation, ripple = design(sections, edge, restarts)
        largest = max(coefficients)
        print(
            f"{name:10s} {span:20s} {attenuation:6.2f} dB  "
            f"ripple {ripple:.1e} dB  max coefficient {largest:.3f}"
        )
        if attenuation < TARGET_DB:
            raise SystemExit(f"{name} reached only {attenuation:.2f} dB")

        body = "".join(f"    {as_f32_literal(value)},\n" for value in coefficients)
        lines += [
            f"/// The {span} halfband: **{attenuation:.1f} dB** in the stopband",
            f"/// (from {edge:.4f} of pi), passband ripple {ripple:.1e} dB.",
            f"pub const {name}: [f32; {sections}] = [",
            body.rstrip("\n"),
            "];",
            "",
        ]

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text("\n".join(lines))
    print(f"wrote {OUTPUT}")


if __name__ == "__main__":
    main()
