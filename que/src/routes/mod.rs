// macro_rules! routes {
//     ($($name:ident),*) => {
//         $(
//             mod $name;
//             pub use $name::*;
//         )*
//     };
// }

//routes![leader_board, set_name, signup, submit_game, websocket];

mod leader_board;
pub use leader_board::*;
mod set_name;
pub use set_name::*;
mod signup;
pub use signup::*;
mod submit_game;
pub use submit_game::*;
mod websocket;

pub use websocket::*;
