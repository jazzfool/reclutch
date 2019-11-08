// source of this idea:
//  https://github.com/stjepang/async-std/blob/832c70aa0e5f8b156558c988b2550cc21130a79e/src/utils.rs

#[doc(hidden)]
#[macro_export]
macro_rules! channels_api {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "crossbeam-channel")]
            #[cfg_attr(feature = "docs", doc(cfg(channels)))]
            $item
        )*
    }
}
