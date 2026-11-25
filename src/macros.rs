// Inner implementation of the variadic_generic macro.
macro_rules! variadic_generic_recursive {
    // Case 1: Non-empty parameter lists
    (
        $impl_macro:ident,
        ($($macro_args:tt)*),
        [$num_args:tt, $($tail_num_args:tt,)*],
        [$head_id_ty:ident, $($tail_id_tys:ident,)*] ($($id_tys:ident,)*),
        [$head_tuple_idx:tt, $($tail_tuple_idx:tt,)*] ($($tuple_idx:tt,)*),
        [$head_arg:ident: $head_ty:ty, $($tail_args:ident: $tail_tys:ty,)*]($($args:ident: $tys:ty,)*)
    ) => {
        $impl_macro!(
            $num_args,
            ($($id_tys,)*),
            ($($tuple_idx,)*),
            ($($args: $tys,)*),
            $($macro_args)*
        );

        $crate::macros::variadic_generic_recursive!(
            $impl_macro,
            ($($macro_args)*),
            [$($tail_num_args,)*],
            [$($tail_id_tys,)*] ($($id_tys,)* $head_id_ty,),
            [$($tail_tuple_idx,)*] ($($tuple_idx,)* $head_tuple_idx,),
            [$($tail_args: $tail_tys,)*] ($($args: $tys,)* $head_arg: $head_ty,)
        );
    };

    // Case 2: Exhausted parameter lists
    (
        $impl_macro:ident,
        ($($macro_args:tt)*),
        [$num_args:tt,],
        [] ($($id_tys:ident,)*),
        [] ($($tuple_idx:tt,)*),
        [] ($($args:ident: $tys:ty,)*)
    ) => {
        $impl_macro!(
            $num_args,
            ($($id_tys,)*),
            ($($tuple_idx,)*),
            ($($args: $tys,)*),
            $($macro_args)*
        );
    };
}

pub(crate) use variadic_generic_recursive;

cfg_select! {
    feature = "more_args_24" => {
        macro_rules! variadic_generic {
            ($impl_macro:ident! [$($args:tt)*]) => {
                $crate::macros::variadic_generic!($impl_macro!($($args)*))
            };

            ($impl_macro:ident! { $($args:tt)* }) => {
                $crate::macros::variadic_generic!($impl_macro!($($args)*))
            };

            ($impl_macro:ident! ($($args:tt)*)) => {
                $crate::macros::variadic_generic_recursive!(
                    $impl_macro,
                    ($($args)*),
                    [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,],
                    [
                        T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,T12,
                        T13,T14,T15,T16,T17,T18,T19,T20,T21,T22,T23,
                    ](),
                    [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,](),
                    [
                        a0:T0,a1:T1,a2:T2,a3:T3,a4:T4,a5:T5,
                        a6:T6,a7:T7,a8:T8,a9:T9,a10:T10,a11:T11,
                        a12:T12,a13:T13,a14:T14,a15:T15,a16:T16,a17:T17,
                        a18:T18,a19:T19,a20:T20,a21:T21,a22:T22,a23:T23,
                    ]()
                );
            };
        }
    }
    feature = "more_args_18" => {
        macro_rules! variadic_generic {
            ($impl_macro:ident! [$($args:tt)*]) => {
                $crate::macros::variadic_generic!($impl_macro!($($args)*))
            };

            ($impl_macro:ident! { $($args:tt)* }) => {
                $crate::macros::variadic_generic!($impl_macro!($($args)*))
            };

            ($impl_macro:ident! ($($args:tt)*)) => {
                $crate::macros::variadic_generic_recursive!(
                    $impl_macro,
                    ($($args)*),
                    [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,],
                    [T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,T12,T13,T14,T15,T16,T17,](),
                    [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,](),
                    [
                        a0:T0,a1:T1,a2:T2,a3:T3,a4:T4,a5:T5,
                        a6:T6,a7:T7,a8:T8,a9:T9,a10:T10,a11:T11,
                        a12:T12,a13:T13,a14:T14,a15:T15,a16:T16,a17:T17,
                    ]()
                );
            };
        }
    }
    _ => {
        macro_rules! variadic_generic {
            ($impl_macro:ident! [$($args:tt)*]) => {
                $crate::macros::variadic_generic!($impl_macro!($($args)*))
            };

            ($impl_macro:ident! { $($args:tt)* }) => {
                $crate::macros::variadic_generic!($impl_macro!($($args)*))
            };

            ($impl_macro:ident! ($($args:tt)*)) => {
                $crate::macros::variadic_generic_recursive!(
                    $impl_macro,
                    ($($args)*),
                    [0,1,2,3,4,5,6,7,8,9,10,11,12,],
                    [T0,T1,T2,T3,T4,T5,T6,T7,T8,T9,T10,T11,](),
                    [0,1,2,3,4,5,6,7,8,9,10,11,](),
                    [a0:T0,a1:T1,a2:T2,a3:T3,a4:T4,a5:T5,a6:T6,a7:T7,a8:T8,a9:T9,a10:T10,a11:T11,]()
                );
            };
        }
    }
}

pub(crate) use variadic_generic;
