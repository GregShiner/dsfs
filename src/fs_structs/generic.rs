use std::os::unix::fs::FileExt;

pub trait FsStruct<E: std::error::Error + From<std::io::Error>> {
    fn read(&mut self, file: &std::fs::File, loc: u64) -> Result<(), E>
    where
        Self: std::marker::Sized,
    {
        let mut buf = vec![0u8; std::mem::size_of::<Self>()];
        file.read_exact_at(&mut buf, loc).map_err(|e| E::from(e))?;
        *self = unsafe { std::mem::transmute_copy(&buf.as_slice()) };
        Ok(())
    }

    fn write(&self, file: &std::fs::File, loc: u64) -> Result<(), E>
    where
        Self: std::marker::Sized,
    {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (self as *const Self) as *const u8,
                std::mem::size_of::<Self>(),
            )
        };
        file.write_all_at(bytes, loc).map_err(|e| E::from(e))?;
        Ok(())
    }
}
