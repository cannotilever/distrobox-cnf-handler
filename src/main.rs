use std::cmp::Ordering;
use std::env;
use std::fmt::{Display, Formatter};
use std::io::{self, Error, ErrorKind};
use std::process::{exit, Command, Stdio};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    // sanity check; make sure we are not in a container
    match env::var("CONTAINER_ID") {
        Ok(id) => {
            if !id.trim().is_empty() {
                eprintln!("Cannot run inside a container! {}", id);
                exit(1);
            }
        }
        Err(_) => {}
    }
    let mut boxes: Vec<DistroboxInstance> = match get_boxes() {
        Ok(box_list) => box_list,
        Err(e) => {
            eprintln!("Cannot get boxes: {:?}", e);
            exit(2);
        }
    };
    boxes.sort();
    for box_inst in boxes {
        match Command::new("distrobox-enter")
            .arg(&box_inst.name)
            .arg("--")
            .args(args.clone())
            .stderr(Stdio::null()) // disable error output
            .spawn()
        {
            Ok(mut child) => {
                match child.wait() {
                    Ok(status) => {
                        if status.code() != Some(127) {
                            exit(0);
                        }
                        // else, try next box
                    }
                    Err(_) => {
                        // does not exist in this box, try the next one
                    }
                }
            }
            Err(e) => {
                eprintln!("Cannot run distrobox-enter: {:?}", e);
                exit(1);
            }
        }
    }
    eprintln!("Cannot find {} in any boxes!", args[0]);
    exit(3);
}

struct DistroboxInstance {
    name: String,
    priority: usize,
    running: bool,
}
impl TryFrom<(usize, &String)> for DistroboxInstance {
    type Error = Error;

    fn try_from(value: (usize, &String)) -> Result<DistroboxInstance, Error> {
        let mut split_stat = value.1.split("|");
        Ok(DistroboxInstance {
            name: split_stat
                .nth(1)
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "Name was not found"))?
                .trim()
                .to_string(),
            priority: value.0,
            running: split_stat
                .next()
                .ok_or_else(|| Error::new(ErrorKind::NotFound, "State was not found"))?
                .contains("Up"),
        })
    }
}

impl Display for DistroboxInstance {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Box {} [{}]", self.name, self.priority)
    }
}
impl Eq for DistroboxInstance {}
impl PartialEq<Self> for DistroboxInstance {
    fn eq(&self, other: &Self) -> bool {
        self.priority.eq(&other.priority)
            && self.name.eq(&other.name)
            && self.running.eq(&other.running)
    }
}
impl PartialOrd<Self> for DistroboxInstance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DistroboxInstance {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.running == other.running {
            self.priority.cmp(&other.priority)
        } else {
            if self.running {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
    }

    fn max(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self.priority > other.priority {
            self
        } else {
            other
        }
    }

    fn min(self, other: Self) -> Self
    where
        Self: Sized,
    {
        if self.priority < other.priority {
            self
        } else {
            other
        }
    }

    fn clamp(self, min: Self, max: Self) -> Self
    where
        Self: Sized,
    {
        if self.priority < min.priority {
            min
        } else if self.priority > max.priority {
            max
        } else {
            self
        }
    }
}

fn get_boxes() -> io::Result<Vec<DistroboxInstance>> {
    let out = Command::new("/usr/bin/distrobox-list")
        .arg("--no-color")
        .output()?;
    if !out.status.success() {
        return Err(Error::new(ErrorKind::Other, format!("{:?}", out.status)));
    }
    let result: String = match String::from_utf8(out.stdout) {
        Ok(s) => s,
        Err(_) => {
            return Err(Error::new(ErrorKind::InvalidData, "Bad UTF-8"));
        }
    };
    // parse command output
    let lines: Vec<String> = result.lines().map(|x| x.to_string()).collect();
    let mut boxes: Vec<DistroboxInstance> = vec![];
    for line in lines.iter().enumerate().skip(1) {
        let dbx: DistroboxInstance = DistroboxInstance::try_from(line)?;
        boxes.push(dbx);
    }
    Ok(boxes)
}
