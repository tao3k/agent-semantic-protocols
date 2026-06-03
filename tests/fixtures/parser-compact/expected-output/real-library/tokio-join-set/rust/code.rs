pub struct JoinSet
field inner: IdleNotifiedSet<JoinHandle<T>>

impl JoinSet<T>
fn new
return Self
fn len
return len
fn is_empty
return is_empty

impl JoinSet<T>
fn build_task
return Builder
fn spawn
return insert
fn spawn_on
return insert
fn spawn_local
return insert
fn spawn_local_on
return insert
fn spawn_blocking
return insert
fn spawn_blocking_on
return insert
fn insert
assign abort
assign entry
return with_value_and_context
return abort
async fn join_next
await std::future::poll_fn(| cx | self.poll_join_next(cx))
call std::future::poll_fn
async fn join_next_with_id
await std::future::poll_fn(| cx | self.poll_join_next_with_id(cx))
call std::future::poll_fn
fn try_join_next
loop
assign entry
assign res
if let Poll::Ready(res) = res
assign _entry
return Some(res)
fn try_join_next_with_id
loop
assign entry
assign res
if let Poll::Ready(res) = res
assign entry
return Some(res.map(| output |(entry.id(), output)))
async fn shutdown
return abort_all
while self.join_next().await.is_some()
async fn join_all
assign output
while let Some(res) = self.join_next().await
match res
case Ok(t)

impl Drop for JoinSet<T>
fn drop
return drain

impl fmt::Debug for JoinSet<T>
fn fmt
return finish

impl Default for JoinSet<T>
fn default
return Self::new

impl std::iter::FromIterator<F> for JoinSet<T>
fn from_iter
assign set
return for_each
return set

impl std::iter::Extend<F> for JoinSet<T>
fn extend
return for_each
