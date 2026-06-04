// n-body (benchmarksgame) — Gleam / BEAM. Gleam is immutable, so
// the system is a List(Body) rebuilt each step rather than mutated
// in place. Velocities update from the shared current positions,
// then positions advance. Matches c/go/rust/odin/zo bit-for-bit:
// -0.169075164 then -0.169087605 at N=1000.

import gleam/float
import gleam/int
import gleam/io
import gleam/list

const days_per_year = 365.24

// 4 * pi * pi.
const solar_mass = 39.47841760435743

type Body {
  Body(
    x: Float,
    y: Float,
    z: Float,
    vx: Float,
    vy: Float,
    vz: Float,
    mass: Float,
  )
}

fn bodies() -> List(Body) {
  [
    Body(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, solar_mass),
    Body(
      4.8414314424647209,
      -1.16032004402742839,
      -0.103622044471123109,
      0.00166007664274403694 *. days_per_year,
      0.00769901118419740425 *. days_per_year,
      -0.0000690460016972063023 *. days_per_year,
      0.000954791938424326609 *. solar_mass,
    ),
    Body(
      8.34336671824457987,
      4.12479856412430479,
      -0.403523417114321381,
      -0.00276742510726862411 *. days_per_year,
      0.00499852801234917238 *. days_per_year,
      0.0000230417297573763929 *. days_per_year,
      0.000285885980666130812 *. solar_mass,
    ),
    Body(
      12.894369562139131,
      -15.1111514016986312,
      -0.223307578892655734,
      0.00296460137564761618 *. days_per_year,
      0.0023784717395948095 *. days_per_year,
      -0.0000296589568540237556 *. days_per_year,
      0.0000436624404335156298 *. solar_mass,
    ),
    Body(
      15.3796971148509165,
      -25.9193146099879641,
      0.179258772950371181,
      0.00268067772490389322 *. days_per_year,
      0.00162824170038242295 *. days_per_year,
      -0.000095159225451971587 *. days_per_year,
      0.0000515138902046611451 *. solar_mass,
    ),
  ]
}

fn velocity(bi: Body, i: Int, indexed: List(#(Int, Body)), dt: Float) -> Body {
  let delta =
    list.fold(indexed, #(0.0, 0.0, 0.0), fn(acc, entry) {
      let #(j, bj) = entry

      case i == j {
        True -> acc
        False -> {
          let dx = bj.x -. bi.x
          let dy = bj.y -. bi.y
          let dz = bj.z -. bi.z
          let d2 = dx *. dx +. dy *. dy +. dz *. dz
          let assert Ok(dist) = float.square_root(d2)
          let mag = dt /. { d2 *. dist }
          let #(ax, ay, az) = acc

          #(
            ax +. dx *. bj.mass *. mag,
            ay +. dy *. bj.mass *. mag,
            az +. dz *. bj.mass *. mag,
          )
        }
      }
    })

  let #(dvx, dvy, dvz) = delta

  Body(..bi, vx: bi.vx +. dvx, vy: bi.vy +. dvy, vz: bi.vz +. dvz)
}

fn advance(bs: List(Body), dt: Float) -> List(Body) {
  let indexed = list.index_map(bs, fn(b, i) { #(i, b) })

  bs
  |> list.index_map(fn(b, i) { velocity(b, i, indexed, dt) })
  |> list.map(fn(b) {
    Body(..b, x: b.x +. dt *. b.vx, y: b.y +. dt *. b.vy, z: b.z +. dt *. b.vz)
  })
}

fn energy(bs: List(Body)) -> Float {
  let indexed = list.index_map(bs, fn(b, i) { #(i, b) })

  list.fold(indexed, 0.0, fn(e, entry) {
    let #(i, bi) = entry
    let speed2 = bi.vx *. bi.vx +. bi.vy *. bi.vy +. bi.vz *. bi.vz
    let kinetic = 0.5 *. bi.mass *. speed2

    let potential =
      list.fold(indexed, 0.0, fn(p, other) {
        let #(j, bj) = other

        case j > i {
          True -> {
            let dx = bi.x -. bj.x
            let dy = bi.y -. bj.y
            let dz = bi.z -. bj.z
            let assert Ok(dist) =
              float.square_root(dx *. dx +. dy *. dy +. dz *. dz)

            p -. bi.mass *. bj.mass /. dist
          }
          False -> p
        }
      })

    e +. kinetic +. potential
  })
}

fn offset_momentum(bs: List(Body)) -> List(Body) {
  let #(px, py, pz) =
    list.fold(bs, #(0.0, 0.0, 0.0), fn(acc, b) {
      let #(x, y, z) = acc

      #(x +. b.vx *. b.mass, y +. b.vy *. b.mass, z +. b.vz *. b.mass)
    })

  case bs {
    [sun, ..rest] -> [
      Body(
        ..sun,
        vx: 0.0 -. px /. solar_mass,
        vy: 0.0 -. py /. solar_mass,
        vz: 0.0 -. pz /. solar_mass,
      ),
      ..rest
    ]
    [] -> bs
  }
}

fn run(bs: List(Body), n: Int) -> List(Body) {
  case n {
    0 -> bs
    _ -> run(advance(bs, 0.01), n - 1)
  }
}

pub fn main() {
  let initial = offset_momentum(bodies())

  io.println(float.to_string(energy(initial)))

  let evolved = run(initial, 1000)

  io.println(float.to_string(energy(evolved)))
}
