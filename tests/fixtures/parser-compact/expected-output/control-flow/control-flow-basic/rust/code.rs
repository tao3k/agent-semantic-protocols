pub fn decide(flag: bool, values: &[i32]) -> Option<i32>
let mut total = 0
for value in values
if * value<0
return None
assign total += * value
if flag &&total> 10
call Some
else
call Some
