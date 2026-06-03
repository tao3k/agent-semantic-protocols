pub struct Searcher
field config: Config
field decode_builder: DecodeReaderBytesBuilder
field decode_buffer: RefCell<Vec<u8>>
field line_buffer: RefCell<LineBuffer>
field multi_line_buffer: RefCell<Vec<u8>>

impl Searcher
fn new
return build
fn search_path
assign path
assign file
return search_file_maybe_path
fn search_file
return search_file_maybe_path
fn search_file_maybe_path
if let Some(mmap) = self.config.mmap.open(file, path)
call log::trace!("{:?}: searching via memory map", path)
return self.search_slice(matcher,&mmap, write_to)
if self.multi_line_with_matcher(&matcher)
call log::trace!("{:?}: reading entire file on to heap for mulitline", path)
try self.fill_multi_line_buffer_from_file::<S>(file)
call fill_multi_line_buffer_from_file
call log::trace!("{:?}: searching via multiline strategy", path)
return run
else
call log::trace!("{:?}: searching using generic reader", path)
return search_reader
fn search_reader
try self.check_config(&matcher).map_err(S::Error::error_config)
call map_err
assign decode_buffer
assign decoder
if self.multi_line_with_matcher(&matcher)
call log::trace!("generic reader: reading everything to heap for multiline")
try self.fill_multi_line_buffer_from_reader::<_, S>(decoder)
call fill_multi_line_buffer_from_reader
call log::trace!("generic reader: searching via multiline strategy")
return run
else
assign line_buffer
assign rdr
call log::trace!("generic reader: searching via roll buffer strategy")
return run
fn search_slice
try self.check_config(&matcher).map_err(S::Error::error_config)
call map_err
if self.slice_needs_transcoding(slice)
call log::trace!("slice reader: needs transcoding, using generic reader")
return self.search_reader(matcher, slice, write_to)
if self.multi_line_with_matcher(&matcher)
call log::trace!("slice reader: searching via multiline strategy")
return run
else

impl Searcher
fn line_terminator
fn binary_detection
fn invert_match
fn line_number
fn multi_line
fn stop_on_nonmatch
fn max_matches
fn multi_line_with_matcher
if ! self.multi_line()
return false
if let Some(line_term) = matcher.line_terminator()
if line_term == self.line_terminator()
return false
if let Some(non_matching) = matcher.non_matching_bytes()
if non_matching.contains(self.line_terminator().as_byte())
return false
return true
fn after_context
fn before_context
fn passthru
fn fill_multi_line_buffer_from_file
call assert!(self.config.multi_line)
assign decode_buffer
assign read_from
if self.config.heap_limit.is_none()
assign buf
return clear
assign cap
return reserve
try read_from.read_to_end(&mut *buf).map_err(S::Error::error_io)
call map_err
return Ok(())
return fill_multi_line_buffer_from_reader
fn fill_multi_line_buffer_from_reader
call assert!(self.config.multi_line)
assign buf
return clear
assign heap_limit
match self.config.heap_limit
case Some(heap_limit)
case None
try read_from.read_to_end(&mut *buf).map_err(S::Error::error_io)
call map_err
return Ok(())
if heap_limit == 0
return Err(S::Error::error_io(alloc_error(heap_limit)))
return resize
