//! 读取层：把成员的字节取出来交给计算层，两层之间隔着一道**有界通道**。
//!
//! 分层的理由在介质上（ADR 0009，见 [`crate::medium`]）：读取的最优并发度由盘决定，
//! 计算的由核数决定，两个数不相干。合成一层就只能取一个，而机械盘上那个数是 1——
//! 那等于把固态盘上的计算也按机械盘的节奏发牌。
//!
//! 通道按**在途字节**背压，不按条数：页的大小差着一个量级（几十 KB 到几 MB），
//! 按条数限等于没限。闸开在**读之前**，因此在途字节从不越过预算，
//! 峰值内存不随读取并发度线性增长（13 号票）。
//!
//! 出来的[一份读取](Read)按成员序号**有序**交付。计算层其实不在乎顺序——每一份都带着自己的
//! 序号，乱序也归得了位；有序是为了另一个调用方：幂等那一道要按成员次序喂哈希
//! （见 `crate::metadata` 的 `SourceHasher`），而它没有第二种喂法。
//! 一套读取层供两处用，比两套各自为政要省心。
//!
//! **并发那一支不问容器是什么**：每条读取线程拿一份自己的[读取端](Reader::independent)——
//! 目录卷是抄一份卷根，归档卷是另开一个文件句柄。因此「一个 ZipArchive 就是一个游标」
//! 拦住的只是**共用**那一个游标，各开各的不在此列。派不派得动并发由
//! [读取计划](crate::medium::IoPlan)定，这一层照它做（`CONTEXT.md` 的《I/O 与并发》）。
//! 一趟因此同时开着几个句柄，见 [`Reader`] 的《一趟同时开着几个句柄》——那是那笔账的
//! 唯一出处，这一层只是它带乘数的那一格。

use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;

use anyhow::Result;

use crate::cost;
use crate::source::{Member, Reader};

/// 在途字节的预算：读取层最多让这么多源字节同时待在通道里。
///
/// 64 MiB。B 类页的源文件中位约 400 KB（measurements 的《B 类素材普查》：12 GB / 30,576 张），
/// 这个数因此装得下一百多页——读取跑在计算前面绰绰有余。而它是个**常数**：
/// 并发度翻倍不会让峰值跟着翻倍，那正是有界通道要买的东西。
///
/// 它不与 `--cache-budget` 共用一个数。那个限的是两遍之间存下的**参照**（ADR 0005），
/// 这个限的是还没解码的**源字节**；两者在管线上前后不相接，同一个数没有意义，
/// 而把它们绑在一起会让「限制内存占用」这个旋钮同时动两个不相干的量。
pub const BUDGET: u64 = 64 * 1024 * 1024;

/// 一个成员读出来的结果，连同它在成员表里的序号。
pub struct Read {
    /// 这个成员在传给 [`reads`] 的那张表里的序号。计算层乱序完成，靠它归位。
    pub index: usize,
    /// 读出来的字节，或者读不出来的那句话。
    ///
    /// 读不出来在这里**不是错误，是一个结果**：它在第一遍里变成失败页（12 号票），
    /// 而一页读不出来不该毁掉整卷。把它做成 `Result<Read>` 就等于让读取层替上层做那个决定。
    pub bytes: Result<Vec<u8>>,
    /// 在途字节的占位。它一放手，读取层就允许再读进来一份。
    ///
    /// 因此**把这一份整个拿着直到算完**不是可有可无的礼节：提前把它拆开丢掉，
    /// 背压就从「已读未算完」松成「已读」，通道白限。字段私有正是为了这一条——
    /// 拆解 [`Read`] 的唯一方法是让它整个走出作用域。
    _permit: Permit,
}

/// 按计划读一批成员。
///
/// `readers` 是[读取计划](crate::medium::IoPlan)定下的并发度，1 即串行。归档卷上
/// 两遍那一路仍恒为 1，幂等那一道不是——两个数各由计划里的一格给出，这一层只照它做。
///
/// **开不出那么多读取端就用开出来的那几条。**归档卷每条线程要一个自己的文件句柄
/// （见 [`Reader::independent`]），句柄开不出来时少几条读取只是慢一点，
/// 而在这里当场失败会毁掉一整卷——读取层的规矩是「读不出来是一个结果，不是一个错误」，
/// 开不出句柄同理。**开出的不到两条就退回串行**：一条时并发与串行本来就是同一件事，
/// 而一条都没开出来时并发那一支根本走不动（没有线程去领号，取的一端会一直等）。
///
/// 降下来的这个条数**不进报告**：报告印的是[读取计划](crate::medium::IoPlan)——
/// 定下来要派几条，以及那个数是谁定的。真派出去几条是这一层的实况，两者可以不等，
/// 而这条分岔**只在异常路径上**：一趟同时要的句柄够不着任何平台的上限
/// （数与式子见 [`Reader`] 的《一趟同时开着几个句柄》），因此没有哪道上限会让这两个数
/// 按设计常态分家。
/// 真降下来了，多半连字节也读不出来，那一卷横竖全员读不出（`p2-loose-ends/12` 判过这一条）。
pub fn reads<'a>(
    reader: &'a mut Reader,
    members: &'a [&'a Member],
    readers: usize,
    budget: u64,
) -> Reads<'a> {
    let throttle = Arc::new(Throttle::new(
        budget,
        members.iter().map(|member| member.bytes),
    ));
    // 比成员还多的读取线程是白开的：它们生下来就领不到号。
    let readers = readers.min(members.len());
    if readers > 1 {
        let mut own: Vec<Reader> = Vec::with_capacity(readers);
        while own.len() < readers {
            match reader.independent() {
                Ok(opened) => own.push(opened),
                Err(_) => break,
            }
        }
        if own.len() > 1 {
            // 成员表要整个搬进线程里：线程活得比这次借用长，借不过去（见 `Member` 的《可克隆》）。
            let owned: Arc<Vec<Member>> =
                Arc::new(members.iter().map(|member| (*member).clone()).collect());
            return Reads::Concurrent(Concurrent::spawn(own, owned, throttle));
        }
    }
    Reads::Serial {
        reader,
        members,
        throttle,
    }
}

/// 一批成员的字节，按序号有序交付。
///
/// **取的一端同时攥在手里的不能超过预算。**这不是建议：闸开在读之前，攥着不放就等于占着在途
/// 字节不还，攥满一整个预算之后读取层再也读不进来，而取的一端还在等下一份——两边互相等。
/// 正常的用法本来就不会撞上它（拿一份、算完、丢掉，rayon 那一侧每条线程手里只有一份），
/// 撞上它的是「先 `collect` 成一个 `Vec` 再算」这种写法，而那正是有界通道要拦掉的东西。
pub enum Reads<'a> {
    /// 串行：在取的那一刻现读。不另起线程——读取与计算的重叠由 rayon 那一侧给出，
    /// 而一条读取上的页本来就该一页一页地读。
    Serial {
        reader: &'a mut Reader,
        members: &'a [&'a Member],
        throttle: Arc<Throttle>,
    },
    /// 并发：几条读取线程同时在读，读完的先到通道里排队，取的那一端按序号放行。
    Concurrent(Concurrent),
}

impl Iterator for Reads<'_> {
    type Item = Read;

    fn next(&mut self) -> Option<Read> {
        match self {
            Reads::Serial {
                reader,
                members,
                throttle,
            } => {
                let claim = throttle.claim()?;
                let bytes = cost::stage(cost::Stage::Read, || reader.read(members[claim.index]));
                Some(Read {
                    index: claim.index,
                    bytes,
                    _permit: claim.permit,
                })
            }
            Reads::Concurrent(concurrent) => concurrent.next(),
        }
    }
}

impl Reads<'_> {
    /// 在途字节到过的最高点。背压那一条靠它量得出来。
    ///
    /// 只有用例问它：管线上没有哪一步要看这个数，而它是「有界通道真的有界」唯一
    /// 从外面量得到的形式。
    #[cfg(test)]
    pub fn peak_in_flight(&self) -> u64 {
        let throttle = match self {
            Reads::Serial { throttle, .. } => throttle,
            Reads::Concurrent(concurrent) => &concurrent.throttle,
        };
        throttle.lock().peak
    }
}

/// 并发那一支的家当。
pub struct Concurrent {
    throttle: Arc<Throttle>,
    /// 读完了的那些，先到先排。
    done: Receiver<Read>,
    /// 到得早了的那些：序号还没轮到它，先在这儿等着。
    ///
    /// 它占着的字节仍算在途——因此这个缓冲同样受预算约束，不会因为一页读得慢就把后面
    /// 整卷囤进内存。
    early: BTreeMap<usize, Read>,
    /// 下一个该交出去的序号。
    next: usize,
    total: usize,
    workers: Vec<JoinHandle<()>>,
}

impl Concurrent {
    /// `readers` 一条线程一份，各读各的（见 [`Reader::independent`]）。
    fn spawn(readers: Vec<Reader>, members: Arc<Vec<Member>>, throttle: Arc<Throttle>) -> Self {
        let total = members.len();
        let (sender, done) = channel();
        let workers = readers
            .into_iter()
            .map(|mut reader| {
                let members = Arc::clone(&members);
                let throttle = Arc::clone(&throttle);
                let sender: Sender<Read> = sender.clone();
                std::thread::spawn(move || {
                    while let Some(claim) = throttle.claim() {
                        let bytes =
                            cost::stage(cost::Stage::Read, || reader.read(&members[claim.index]));
                        let read = Read {
                            index: claim.index,
                            bytes,
                            _permit: claim.permit,
                        };
                        // 送不出去说明取的那一端已经走了：这一趟不必读完。
                        if sender.send(read).is_err() {
                            break;
                        }
                    }
                })
            })
            .collect();
        // 手里这一个发送端要丢掉，否则通道永远关不上。
        drop(sender);
        Self {
            throttle,
            done,
            early: BTreeMap::new(),
            next: 0,
            total,
            workers,
        }
    }

    fn next(&mut self) -> Option<Read> {
        loop {
            if self.next >= self.total {
                return None;
            }
            if let Some(read) = self.early.remove(&self.next) {
                self.next += 1;
                return Some(read);
            }
            match self.done.recv() {
                Ok(read) => {
                    self.early.insert(read.index, read);
                }
                // 读取线程全退了而序号还没走完：只有闸被关过才会这样（见 [`Throttle::stop`]）。
                Err(_) => return None,
            }
        }
    }
}

impl Drop for Concurrent {
    fn drop(&mut self) {
        // 关闸，等在闸上的线程才走得动；早到的那些也放掉，它们占着的字节要还回去。
        self.throttle.stop();
        self.early.clear();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

/// 在途字节的闸：读取层每读一份先在这里占住它的字节数，计算层用完了才放回来。
///
/// 这是「有界通道按字节预算背压」那一条的落点（13 号票）。
///
/// **领序号与占字节在同一把锁里**，这一条不是顺手写成的：序号因此恒按序被领走，
/// 而「领到号」蕴含「占到了字节」。于是还没交付的那些序号里最小的那个必然已经在读了——
/// 不会出现「A 领了 0 号却卡在闸上，B 领了 1 号占满了预算」这种谁也走不动的局面。
///
/// 在途为零时无条件放行：单页大过整个预算的卷否则会当场死锁。预算因此是
/// 「预算 + 最大的那一页」的软上界，不是硬上界——但它仍与并发度无关，那才是要点。
pub struct Throttle {
    budget: u64,
    /// 每个成员有多少字节，序号即下标。
    sizes: Vec<u64>,
    state: Mutex<ThrottleState>,
    /// 有字节还回来了，或者闸关了。
    room: Condvar,
}

struct ThrottleState {
    /// 下一个还没被谁领走的序号。
    next: usize,
    /// 在途字节：已占住、还没还回来的那些。
    in_flight: u64,
    /// 在途字节到过的最高点。
    peak: u64,
    /// 关了闸就不再发号，等在上面的线程一并放走。
    stopped: bool,
}

/// 领到的一号：序号，加上它占住的那份字节。
struct Claim {
    index: usize,
    permit: Permit,
}

impl Throttle {
    fn new(budget: u64, sizes: impl Iterator<Item = u64>) -> Self {
        Self {
            budget,
            sizes: sizes.collect(),
            state: Mutex::new(ThrottleState {
                next: 0,
                in_flight: 0,
                peak: 0,
                stopped: false,
            }),
            room: Condvar::new(),
        }
    }

    /// 锁上闸的状态。
    ///
    /// 中毒了照样用：里面只有几个计数器，一个读取线程恐慌不该让其余每一条跟着恐慌——
    /// 那会把一处失败放大成整趟失败。
    fn lock(&self) -> MutexGuard<'_, ThrottleState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// 领下一个序号，预算不够就等。没号可领了（读完了，或闸关了）就是 `None`。
    fn claim(self: &Arc<Self>) -> Option<Claim> {
        let mut state = self.lock();
        loop {
            if state.stopped || state.next >= self.sizes.len() {
                return None;
            }
            let bytes = self.sizes[state.next];
            if state.in_flight == 0 || state.in_flight + bytes <= self.budget {
                let index = state.next;
                state.next += 1;
                state.in_flight += bytes;
                state.peak = state.peak.max(state.in_flight);
                return Some(Claim {
                    index,
                    permit: Permit {
                        throttle: Arc::clone(self),
                        bytes,
                    },
                });
            }
            state = self
                .room
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    /// 还回一份字节。
    fn release(&self, bytes: u64) {
        self.lock().in_flight -= bytes;
        // 唤醒全部而不是一个：等着的各自要的量不同，叫醒的那一个未必就是装得下的那一个。
        self.room.notify_all();
    }

    /// 关闸：不再发号，等在上面的一并放走。
    fn stop(&self) {
        self.lock().stopped = true;
        self.room.notify_all();
    }
}

/// 一份在途字节的占位。走出作用域即还回去。
struct Permit {
    throttle: Arc<Throttle>,
    bytes: u64,
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.throttle.release(self.bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source;

    /// 一个装着 `sizes` 里那些大小的页的目录卷。名字按序号排，阅读顺序即数组顺序。
    fn volume(sizes: &[usize]) -> (tempfile::TempDir, source::Volume) {
        let root = tempfile::tempdir().expect("建卷目录");
        for (index, &size) in sizes.iter().enumerate() {
            let name = format!("{index:03}.png");
            // 内容是序号那个字节铺满，取回来一眼认得出是哪一页。
            std::fs::write(root.path().join(name), vec![index as u8; size]).expect("写页");
        }
        let volume = source::open(root.path()).expect("打开卷");
        (root, volume)
    }

    /// 同上，装成一个归档卷。页的内容与 [`volume`] 那一份逐字节相同。
    ///
    /// 成员**不压缩**（`Stored`）：本模块要量的是交付的次序与字节，不是 deflate。
    fn archive(sizes: &[usize]) -> (tempfile::TempDir, source::Volume) {
        let dir = tempfile::tempdir().expect("建目录");
        let path = dir.path().join("卷一.cbz");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).expect("建归档"));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (index, &size) in sizes.iter().enumerate() {
            writer
                .start_file(format!("{index:03}.png"), options)
                .expect("起一个成员");
            std::io::Write::write_all(&mut writer, &vec![index as u8; size]).expect("写成员");
        }
        writer.finish().expect("收尾归档");
        let volume = source::open(&path).expect("打开归档卷");
        (dir, volume)
    }

    /// 逐页取一遍，顺便把在途字节的峰值记下来。
    fn drain(reads: &mut Reads<'_>) -> Vec<(usize, Vec<u8>)> {
        let mut taken = Vec::new();
        for read in reads.by_ref() {
            taken.push((read.index, read.bytes.expect("这一页读得出来")));
        }
        taken
    }

    /// 串行与并发交出的是同一批字节，且都按成员序号有序——**目录卷与归档卷都是**。
    ///
    /// 有序这一条是幂等那一道的前提：源哈希按成员次序喂，乱一位整卷的指纹就变了。
    /// 归档卷也在这条用例里，是 11 号票的硬约束：并行的是**解**，不是**喂**。
    #[test]
    fn serial_and_concurrent_hand_over_the_same_bytes_in_reading_order() {
        let sizes: Vec<usize> = (0..24).map(|index| 1024 + index * 97).collect();
        for (label, mut opened) in [("目录卷", volume(&sizes)), ("归档卷", archive(&sizes))] {
            let volume = &mut opened.1;
            let members: Vec<&Member> = volume.pages.iter().collect();

            let mut serial = reads(&mut volume.reader, &members, 1, BUDGET);
            let taken = drain(&mut serial);
            drop(serial);
            let mut concurrent = reads(&mut volume.reader, &members, 8, BUDGET);
            let also = drain(&mut concurrent);
            drop(concurrent);

            assert_eq!(taken.len(), sizes.len(), "{label}");
            for (index, (order, bytes)) in taken.iter().enumerate() {
                assert_eq!(*order, index, "{label}第 {index} 份的序号不对");
                assert_eq!(bytes.len(), sizes[index], "{label}第 {index} 份的长度不对");
                assert!(
                    bytes.iter().all(|&byte| byte == index as u8),
                    "{label}第 {index} 份串了页"
                );
            }
            assert_eq!(taken, also, "{label}并发交出的与串行不是同一批");
        }
    }

    /// 预算是在途字节的界：读取跑在计算前面，但跑不出这个数（13 号票）。
    ///
    /// 取页的一端故意不放手——把取到的攒着不丢，正是「计算层还没算完」的样子。
    /// 峰值因此量的是读取层真的囤了多少，而不是它读得有多快。
    #[test]
    fn the_budget_bounds_what_the_reading_layer_runs_ahead_by() {
        const PAGE: usize = 4096;
        let sizes = [PAGE; 32];
        let budget = (PAGE * 4) as u64;
        let (_dir, mut volume) = volume(&sizes);
        let members: Vec<&Member> = volume.pages.iter().collect();

        let mut taking = reads(&mut volume.reader, &members, 8, budget);
        // 攒着四份不放：闸上于是恰好没有余量，八条读取线程一份都不该再读进来。
        let held: Vec<Read> = (&mut taking).take(4).collect();
        assert_eq!(held.len(), 4);
        assert_eq!(taking.peak_in_flight(), budget, "在途字节越过了预算");
        drop(held);

        // 剩下的照常一份一份地过，全程仍在预算之内。
        assert_eq!(drain(&mut taking).len(), sizes.len() - 4);
        assert_eq!(taking.peak_in_flight(), budget, "在途字节越过了预算");
    }

    /// 并发度翻倍，峰值不跟着翻倍——有界通道要买的就是这一条。
    #[test]
    fn the_peak_does_not_grow_with_the_number_of_readers() {
        const PAGE: usize = 8192;
        let sizes = [PAGE; 64];
        let budget = (PAGE * 3) as u64;
        let (_dir, mut volume) = volume(&sizes);
        let members: Vec<&Member> = volume.pages.iter().collect();

        for readers in [1, 2, 4, 16] {
            let mut taking = reads(&mut volume.reader, &members, readers, budget);
            assert_eq!(drain(&mut taking).len(), sizes.len());
            assert!(
                taking.peak_in_flight() <= budget,
                "{readers} 条读取时在途字节越过了预算"
            );
        }
    }

    /// 单页大过整个预算时它自己走一趟，不是死在闸上。
    ///
    /// 与缓存那一侧同一条规矩（见 `crate::cache` 的 `store`）：预算限的是「同时几份」，
    /// 不是「一份最大多少」。
    #[test]
    fn a_member_larger_than_the_whole_budget_still_gets_through() {
        let sizes = [64, 4096, 64];
        let (_dir, mut volume) = volume(&sizes);
        let members: Vec<&Member> = volume.pages.iter().collect();

        let mut taking = reads(&mut volume.reader, &members, 4, 128);
        let taken = drain(&mut taking);

        assert_eq!(taken.len(), 3);
        assert_eq!(taken[1].1.len(), 4096);
    }

    /// 读不出来的成员不是错误，是一份带着那句话的结果：整卷照读下去。
    ///
    /// 12 号票的界线在这里保住：一页读不出来变成失败页，不是让读取层把整卷掀了。
    #[test]
    fn a_member_that_cannot_be_read_comes_back_as_a_result_not_an_error() {
        for readers in [1, 4] {
            let (dir, mut volume) = volume(&[128; 4]);
            let members: Vec<&Member> = volume.pages.iter().collect();
            // 第二页当场删掉：读到它的时候文件已经不在了。
            std::fs::remove_file(dir.path().join("001.png")).expect("删掉一页");

            let taking = reads(&mut volume.reader, &members, readers, BUDGET);
            let mut taken: Vec<(usize, bool)> = Vec::new();
            for read in taking {
                taken.push((read.index, read.bytes.is_ok()));
            }

            assert_eq!(
                taken,
                vec![(0, true), (1, false), (2, true), (3, true)],
                "{readers} 条读取时坏成员没有原样交付"
            );
        }
    }

    /// 取的一端半路走人时，读取线程跟着停下——不把整卷读完再收摊。
    #[test]
    fn dropping_the_reads_stops_the_readers() {
        let (_dir, mut volume) = volume(&[4096; 64]);
        let members: Vec<&Member> = volume.pages.iter().collect();

        let mut taking = reads(&mut volume.reader, &members, 4, 8192);
        let first = taking.next().expect("头一份");
        assert_eq!(first.index, 0);
        drop(first);
        // 丢掉读取层：等在闸上的线程要被放走，join 不该挂住。
        drop(taking);

        // 走到这里就说明线程都收了；再开一趟证明卷本身没被弄坏。
        let mut again = reads(&mut volume.reader, &members, 4, BUDGET);
        assert_eq!(drain(&mut again).len(), 64);
    }

    /// 归档卷点名并发就真走并发那一支：每条线程一个自己的句柄（11 号票）。
    ///
    /// 这一层**不判该不该并发**——那是 `crate::medium` 那一层的事（归档卷的两遍恒为一条，
    /// 幂等那一道按介质走）。这里钉的是「点名了就派得动」：从前它在这里被兜底按回串行，
    /// 那条兜底连同「一个游标」那条理由一起没了。
    #[test]
    fn an_archive_takes_the_concurrent_path_when_it_is_asked_to() {
        let (_dir, mut volume) = archive(&[512; 8]);
        let members: Vec<&Member> = volume.pages.iter().collect();

        let mut taking = reads(&mut volume.reader, &members, 8, BUDGET);

        assert!(
            matches!(taking, Reads::Concurrent { .. }),
            "归档卷点名并发还是走了串行那一支"
        );
        assert_eq!(drain(&mut taking).len(), 8);
    }

    /// 并发那一支同时攥着几个读取端：**一条线程一个**，条数是 `min(点名的条数, 成员数)`。
    ///
    /// 归档卷上一个读取端就是一个打开的文件句柄，这一条因此钉的是那笔句柄账里带乘数的
    /// 那一格——整笔账（连同卷自己那一个、自变量、以及为什么没有一道上限）在
    /// `source::Reader` 的《一趟同时开着几个句柄》，这里不复述。
    #[test]
    fn a_concurrent_read_holds_one_reader_per_worker() {
        for (asked, members, expected) in [(8, 24, 8), (32, 4, 4), (2, 2, 2)] {
            let (_dir, mut volume) = archive(&vec![512; members]);
            let list: Vec<&Member> = volume.pages.iter().collect();

            let taking = reads(&mut volume.reader, &list, asked, BUDGET);

            let Reads::Concurrent(concurrent) = &taking else {
                panic!("点名 {asked} 条、{members} 个成员，该走并发那一支")
            };
            assert_eq!(
                concurrent.workers.len(),
                expected,
                "点名 {asked} 条、{members} 个成员"
            );
        }
    }

    /// 派一条就是串行那一支，归档卷也一样：那时并发与串行本来就是同一件事，
    /// 而多开一个句柄是白开的。
    #[test]
    fn one_reader_is_the_serial_path_whatever_the_container() {
        for (_dir, mut volume) in [volume(&[512; 8]), archive(&[512; 8])] {
            let members: Vec<&Member> = volume.pages.iter().collect();

            let taking = reads(&mut volume.reader, &members, 1, BUDGET);

            assert!(matches!(taking, Reads::Serial { .. }));
        }
    }

    /// 一个成员都没有的卷：一份都不交，也不挂住。
    #[test]
    fn a_volume_with_no_members_hands_over_nothing() {
        let dir = tempfile::tempdir().expect("建目录");
        let mut volume = source::open(dir.path()).expect("打开空卷");
        let members: Vec<&Member> = volume.pages.iter().collect();
        assert!(members.is_empty());

        let mut taking = reads(&mut volume.reader, &members, 8, BUDGET);

        assert!(taking.next().is_none());
    }
}
