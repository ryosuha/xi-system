extern crate dirs;
extern crate websrv;
extern crate rand;
use websrv::ThreadPool;

use std::str;
use std::path::{Path,PathBuf};
use std::io::prelude::*;
use std::net::{TcpStream,TcpListener,Shutdown};
use std::fs::File;
use std::sync::Mutex;
use std::{thread, time};
use std::time::Duration;
use regex::Regex;
use content_inspector::{ContentType, inspect};
use std::process::{Command, Stdio};
use uuid::Uuid;
use rand::Rng;
use wait_timeout::ChildExt;



//----------DEFAULT PARAMETER----------//
const MAX_POOL_THREAD: usize = 8;
const BUFFER: usize = 10000;     //request read buffer
const DEBUG: bool = false;       //debug configuration
const DEBUGDATA: bool = false;       //debug data content configuration
const MU_TIMEOUT: u64 = 5;    //as seconds

#[macro_use]
extern crate lazy_static;

lazy_static!(
    static ref DEBUG_MODE: Mutex<bool> = Mutex::new(DEBUG);
    /*
    How to print a debug message sample
    println!("{}",print_debug());
    or
    if print_debug() { println!("DEBUG 0001 : {:?}",&homepath); }
    */
);
lazy_static!(
    static ref DEBUGDATA_MODE: Mutex<bool> = Mutex::new(DEBUGDATA);
    /*
    How to print a debug message sample
    println!("{}",print_debug());
    or
    if print_debug() { println!("DEBUG 0001 : {:?}",&homepath); }
    */
);

lazy_static! {
    static ref CONLEN: Regex = Regex::new("^Content-Length.*").unwrap();
}

lazy_static! {
    static ref DATABOUNDARY: Regex = Regex::new("multipart/form-data").unwrap();   
}

lazy_static! {
    static ref ENDHTTPHEADER: Regex = Regex::new("\r\n\r\n").unwrap();   
}

lazy_static! {
    static ref DOUBLE_RN: Regex = Regex::new(r"\r\n\r\n").unwrap();   
}

lazy_static! {
    static ref GETCONTAINER_IMAGE_ID: Regex = Regex::new("Successfully built").unwrap();   
}

lazy_static! {
    static ref MAKING_SERVING_MANIFEST_MATCHUUID: Regex = Regex::new(r"\{my_uuid.to_string\(\)\}").unwrap();   
}

lazy_static! {
    static ref MAKING_SERVING_MANIFEST_MATCHCNTID: Regex = Regex::new(r"\{containerid\}").unwrap();   
}

//----------GLOBAL PARAMETER----------//
//nu-system parameter
lazy_static! {
    static ref NU_SYSTEM_DOMAIN: String = String::from("default.dev.mu-system.bokuno.pw");
}

//nu-system's registry fqdn
lazy_static! {
    static ref NU_SYSTEM_REGISTRY: String = String::from("registry.bokuno.pw");
}

fn main() {
    let mut basepath = PathBuf::new();

    //test changing debug mode change_debug();
    //TODO Read configuration file to change debug configuration
    change_debug(true);
    change_debugdata(false);

    //Preparing directory path
    //basepath.push(dirs::home_dir().unwrap());
    basepath.push("/home/ryo/mu-system/websrv/html");

    if print_debug() { println!("DEBUG 0000 : {:?}",&basepath); }
    
    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
    let pool = ThreadPool::new(MAX_POOL_THREAD);

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        let homepath = PathBuf::from(&basepath);
        
        pool.execute(|| {
            handle_connection(stream,homepath);
        });
    }

    println!("Shutting Down Server.Unexpected Behaviour");
}


fn handle_connection(mut stream: TcpStream,homepath: PathBuf) {
    let mut buffer = [0; BUFFER]; //Read buffer
    let mut truncated_buffer: Vec<u8> = vec![];
    let mut largebuffer: Vec<u8> = vec![]; //Read large buffer e.g file in memory
    let mut largebuffer_len = 0usize; //read large buffer size
    let mut header_len = 0usize; //read header size
    let mut data_buffer: Vec<u8> = vec![]; //for data
    let mut real_data_buffer: Vec<u8> = vec![]; //for real_data
    let mut regexp_buffer: Vec<u8> = vec![]; //for searching buffer
    let mut buffer_replace: Vec<u8> = vec![]; //for temporary buffer on extracting binary data on buffer_tmp
    let mut boundarystring = String::from("");
    
    if print_debug() { println!("==========NEW REQUEST==============="); }
    if print_debug() { println!("DEBUG 0001 : {:?}",&homepath); }


    //stream.read(&mut buffer).unwrap();
    //TEST recursive buffer
    let mut count = 1;
    let mut conlensize_int = 0usize;
    let mut data_bytes_start = 0;
    let mut debugpointer: regex::Match;
    let mut is_boundary = false;
    let mut is_content = false;
    let mut is_data = false;
    let mut is_binary = false;
    let mut http_end = false;

    loop {
        match stream.read(&mut buffer){
            Ok(size) => {
                largebuffer.extend(&buffer[0..size]);
                largebuffer_len = largebuffer_len + size;
                if print_debug() { println!("DEBUG 0044 : largebuffer_len : {}",largebuffer_len); }
                if count == 1 {
                    
                    //prepare cloned buffer for calculating http header length
                    let buffer_cln1 = buffer.clone();
                    let toregexp = str::from_utf8(&buffer_cln1).unwrap();
                    if ENDHTTPHEADER.is_match(toregexp) {
                        let matchbyte = ENDHTTPHEADER.find(toregexp).unwrap();
                        header_len = matchbyte.end();
                        if print_debug() { println!("DEBUG 0035 : Match END HTTP HEADER is {}",matchbyte.end()); }
                    }
                    
                    //prepare cloned buffer for calculating content-length
                    let buffer_cln2 = buffer.clone();
                    let parsedline: Vec<&str> = str::from_utf8(&buffer_cln2).unwrap().split("\r\n").collect();
                    for _i in parsedline.iter() {
                        if DATABOUNDARY.is_match(_i){
                            let parsed: Vec<&str> = _i.split("=").collect();
                            boundarystring = remove_all_whitespace(&parsed[1].to_string());
                            if print_debug() { println!("DEBUG 0036 : Boundarystring is {}",boundarystring); }
                            
                            let regexp = Regex::new(&boundarystring).unwrap();
                            let mut firstdatabyte = regexp.find(toregexp).unwrap();
                            data_bytes_start = firstdatabyte.end();
                            is_boundary = true;
                        }

                        if CONLEN.is_match(_i){
                            let parsed: Vec<&str> = _i.split(":").collect();
                            let conlensize = remove_all_whitespace(&parsed[1].to_string());
                            if print_debug() { println!("DEBUG 0037 : Content-length is {}",conlensize); }
                            conlensize_int = conlensize.parse().unwrap();
                            if print_debug() { println!("DEBUG 0038 : Content-length is {} (int)",conlensize_int); }
                            is_content = true;
                        }  
                    }
                    
                    count = 2;
                }
                
                if largebuffer_len == (conlensize_int + header_len){
                    if print_debug() { println!("DEBUG 0040 : largebuffer_len : {}",largebuffer_len); }

                    //http header length is header_len
                    if print_debug() { println!("DEBUG 0041 : header_len : {}",header_len); }

                    //http data size is conlensize_int
                    if print_debug() { println!("DEBUG 0042 : conlensize : {}",conlensize_int); }

                    //packet length is header_len + conlensize_int
                    if print_debug() { println!("DEBUG 0043 : request size : {}",(header_len + conlensize_int)); }
                    break;
                }

                if largebuffer_len == 0 { break; }

            },
            Err(_) => { break; } //error 
        }
        
    }
    if print_debug() { println!("DEBUG 0002 Request: {}", String::from_utf8_lossy(&buffer[..])); }

    //Split HTTP Method(GET/POST/PUT/...)
    let mut buffer_tmp = largebuffer.clone();

    //HINT
    //http header is on 0 to header_len at buffer_tmp
    //data is on header_len+1 to header_len+conlensize_int at buffer_tmp
    //data contain of --boundary_string + content_header + real_data + --boundary_string--
    //first boundary_string begin with two hyphens and last boundary_string is sandwhiched by to hyphens

    //get real data
    if is_boundary && is_content {
        //data_buffer.extend(&largebuffer[(header_len+1)..(header_len+conlensize_int)]);
        data_buffer.extend(&largebuffer[(header_len+1)..]);

        //check is data contain binary or utf8
        if inspect(&data_buffer) == ContentType::BINARY {
            is_binary = true;
            let mut number = str::from_utf8(&data_buffer).unwrap_err();
            real_data_buffer.extend(&data_buffer[(number.valid_up_to())..(data_buffer.len()-(boundarystring.len()+8))]);
            if print_debug() { println!("DEBUG 0051 data_buffer: {}", String::from_utf8_lossy(&real_data_buffer[..])); }
            
            //TODO check is data is mixed utf&binary
            //1.CHECK if there are boundary string
            if ENDHTTPHEADER.is_match(str::from_utf8(&real_data_buffer).unwrap()) {
                if print_debug() { println!("DEBUG 0052 data_buffer: {}", String::from_utf8_lossy(&real_data_buffer[..])); }
            }




        }
        else {
            is_binary = false;

            //TODO check if it mixed utf&binary

            if print_debug() { println!("DEBUG 0045 : data_buffer {}", String::from_utf8_lossy(&data_buffer[..])); }

            //1.DISPOSE until content-header
            let mut matchbyte = ENDHTTPHEADER.find(str::from_utf8(&data_buffer).unwrap()).unwrap();
            real_data_buffer.extend(&data_buffer[(matchbyte.end())..data_buffer.len()]);
            if print_debug() { println!("DEBUG 0046 : data_buffer step-1 {}", String::from_utf8_lossy(&real_data_buffer[..])); }

            //2.DISPOSE last --boundary_string--
            data_buffer.clear();
            data_buffer.extend(&real_data_buffer[0..(real_data_buffer.len()-(boundarystring.len()+8))]);
            if print_debug() { println!("DEBUG 0047 : data_buffer step-2 {}", String::from_utf8_lossy(&data_buffer[..])); }

            //3.Extend real_data_buffer
            real_data_buffer.clear();
            real_data_buffer.extend(&data_buffer[0..data_buffer.len()]);
            if print_debug() { println!("DEBUG 0048 : real_data_buffer {}", String::from_utf8_lossy(&real_data_buffer[..])); }
        }
        //data_buffer should be identical to real_data_buffer

        is_data = true;
    }

    if print_debugdata() { println!("DEBUG 0004 : Copied buffer: {}", String::from_utf8_lossy(&buffer_tmp[..])); }

    //Remove binary data from header packet buffer before continueing
    if inspect(&buffer_tmp) == ContentType::BINARY {
        buffer_replace.clear();
        buffer_replace.extend(&buffer_tmp[0..]);
        let mut number = str::from_utf8(&buffer_tmp).unwrap_err();
        buffer_tmp.clear();
        buffer_tmp.extend(&buffer_replace[0..number.valid_up_to()]);
    }
    let parsedline: Vec<&str> = str::from_utf8(&buffer_tmp).unwrap().split("\r\n").collect();
    let mut parsed: Vec<&str> = parsedline[0].split_whitespace().collect();
    let _parsed_len = parsed.len();
    if print_debug() { println!("DEBUG 0005 : parsed length : {}",_parsed_len); }
    for _i in parsed.iter() {
        if print_debug() { println!("DEBUG 0006 : {}",_i); }
    }

    let mut filename = "/";
    if _parsed_len > 1 {
        filename = parsed[1];
    }
    else {
        filename = "/noresource.html";
    }

    //change requested "/" to "index.html"
    if filename == "/" {
        filename = "index.html";
        parsed[1] = "/index.html";
        
    }
    else {
        //remove first "/"
        filename = remove_first_slash(&filename);
    }

    //reparse request URI
    //example: /aaa/bbb/ccc.html -> {"","aaa","bbb","ccc.html"}
    let mut parsed_uri: Vec<&str> = parsed[1].split("/").collect();
    //remove first null vector -> {"aaa","bbb","ccc.html"}
    parsed_uri.drain(0..1);
    let _parsed_uri_len = parsed_uri.len();
    if print_debug() { println!("DEBUG 0020 : parsed_uri length : {}",_parsed_uri_len); }
    for _i in parsed_uri.iter() {
        if print_debug() { println!("DEBUG 0019 : {}",_i); }
    }

    //Recognizing requested resource or data format
    //aaa/bbb/ccc.html -> "html" file
    //To process data format
    let mut data_format: Vec<&str> = parsed_uri[_parsed_uri_len - 1].split(".").collect();
    let resource_format = match data_format.len() {
        //not a file or no dot on request
        1 => {
            if print_debug() { println!("DEBUG 0026 : requested resource is {}",data_format[0]); }
            "NONE"
        },
        2 => {
            if print_debug() { println!("DEBUG 0027 : requested resource format is {}",data_format[1]); }
            data_format[1]
        },
        _ => {
            if print_debug() { println!("DEBUG 0028 : unable to recognize requested resource format"); }
            "UNKNOWN"
        },
    };

    //let mut _response = String::new();
    let mut _response: Vec<u8> = Vec::new();
    let mut lastresort_request: bool = false;

    //Evaluate API Call
    if print_debug() { println!("DEBUG 0013 : is api call {}",is_api(&filename)); }

    //Process branch for API Call
    if is_api(&filename) {
        if print_debug() { println!("DEBUG 0016 : is api call {}",is_api(&filename)); }
        //Evaluate Request Method
        match parsed[0] {
            "GET" => { 
                if _parsed_uri_len > 1 {
                    if print_debug() { println!("DEBUG 0023 : Matching api resource"); }
                    match parsed_uri[1] {
                        "test_sse.rs" => get_api_testsse(stream,&homepath,&filename),
                        _ => get_api_lastresort(stream,&homepath,&filename),
                    }
                }
                else {
                    get_api_lastresort(stream,&homepath,&filename);
                }
            },
            "POST" => {
                if _parsed_uri_len > 1 {
                    if print_debug() { println!("DEBUG 0023 : Matching api resource"); }
                    match parsed_uri[1] {
                        "filescan.rs" => post_api_filescan(stream,&homepath,&filename,&real_data_buffer),
                        "mu.rs" => post_api_mu(stream,&homepath,&filename,&real_data_buffer),
                        "nu.rs" => post_api_nu(stream,&homepath,&filename,&real_data_buffer),
                        _ => api_lastresort(stream,&homepath,&filename),
                    }
                }
            },
            "PUT" => api_lastresort(stream,&homepath,&filename),
            _ => api_lastresort(stream,&homepath,&filename),
        }
    }

    //Process branch for !API Call
    else{
        //Evaluate Request Method  
        if print_debug() { println!("DEBUG 0056 : method is {}",parsed[0]); }
        if print_debug() { println!("DEBUG 0057 : resource type is {}",resource_format); }
        match parsed[0] {
            "GET" => {
                match resource_format {
                    "html" => _response = get_request(&homepath,&filename).into_bytes(),
                    "jpg" => _response = get_jpg_request(&homepath,&filename),
                    "NONE" => _response = format!("NOT IMPLEMENTED").into_bytes(),
                    "UNKNOWN" => _response = format!("NOT IMPLEMENTED").into_bytes(),
                    _ => _response = format!("NOT IMPLEMENTED").into_bytes(),
                }
            },
            "POST" => _response = format!("NOT IMPLEMENTED").into_bytes(),
            "PUT" => _response = format!("NOT IMPLEMENTED").into_bytes(),
            _ => {
                lastresort_request = true;
                _response = format!("UNDEFINED REQUEST METHOD").into_bytes()
            }
        }

        if lastresort_request {
            stream.shutdown(Shutdown::Both).expect("shutdown call failed");
            if print_debug() { println!("DEBUG 0018 : REQUEST GOES TO LAST RESORT"); }
        }
        else {
            //stream.write(_response.as_bytes()).unwrap();
            stream.write(&_response).unwrap();
            stream.flush().unwrap();
        }
    }

}


//----------BEGIN OF REQUEST ROUTING FUNCTION---------//

//-----------------------------------------------
//Funtion To Process GET Method
//Retrieve homedir as homepath and
//Request URI as filename
//-----------------------------------------------
fn get_request<'a>(homepath: &'a PathBuf,filename: &str) -> String {
    let mut filepath = homepath.clone();
    //Merge homedir with requested URI to point requested file
    if print_debug() { println!("DEBUG 0007 : Filename {}",filename); }
    filepath.push(filename);
    if print_debug() { println!("DEBUG 0008 : Homepath {:?}",filepath); }

    //-----------------------------------------------
    //Check file requested file is found or not.
    //True if found
    //-----------------------------------------------
    if print_debug() { println!("DEBUG 0009 : is file found? {}",file_check(&filepath)); }
    let status_line = if !file_check(&filepath) {
            //filepath.set_file_name("not_found.html");
            filepath = homepath.clone();
            filepath.push("not_found.html");
            if print_debug() { println!("DEBUG 0010 : File Not Found"); }
            "HTTP/1.1 404 NOT FOUND\r\n\r\n"
        }
        else {
            if print_debug() { println!("DEBUG 0011 : File Found"); }
            "HTTP/1.1 200 OK\r\n\r\n"
        };
    
    //-----------------------------------------------
    //Prepare requested response message
    //Read requested file
    //-----------------------------------------------
    
    if print_debug() { println!("DEBUG 0015 : {:?}",filepath); }
    let mut file = File::open(filepath).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    
    format!("{}{}", status_line, contents)

}

fn get_jpg_request(homepath: &PathBuf,filename: &str) -> Vec<u8> {
    let mut filepath = homepath.clone();
    //Merge homedir with requested URI to point requested file
    if print_debug() { println!("DEBUG 0029 : Filename {}",filename); }
    filepath.push(filename);
    if print_debug() { println!("DEBUG 0030 : Homepath {:?}",filepath); }

    //-----------------------------------------------
    //Check file requested file is found or not.
    //True if found
    //-----------------------------------------------
    if print_debug() { println!("DEBUG 0031 : is file found? {}",file_check(&filepath)); }
    let (status_line,flag) = if !file_check(&filepath) {
            //filepath.set_file_name("not_found.html");
            filepath = homepath.clone();
            filepath.push("not_found.html");
            if print_debug() { println!("DEBUG 0032 : File Not Found"); }
            ("HTTP/1.1 404 NOT FOUND\r\n\r\n",0)
        }
        else {
            if print_debug() { println!("DEBUG 0033 : File Found"); }
            ("HTTP/1.1 200 OK\r\n\r\n",1)
        };
    
    //-----------------------------------------------
    //Prepare requested response message
    //Read requested file
    //-----------------------------------------------
    
    if print_debug() { println!("DEBUG 0034 : {:?}",filepath); }
    let mut file = File::open(filepath).unwrap();
    let mut contents: Vec<u8> = Vec::new();
    file.read_to_end(&mut contents).unwrap();

    [status_line.as_bytes().to_vec() , contents].concat()
}

//NOT IMPLEMENTED YET
//-----------------------------------------------
//Funtion To Process POST Method
//Retrieve homedir as homepath and
//Request URI as filename
//-----------------------------------------------
// fn post_request(homepath: &PathBuf,filename: &str) -> &str {}



//NOT IMPLEMENTED YET
//-----------------------------------------------
//Funtion To Process PUT Method
//Retrieve homedir as homepath and
//Request URI as filename
//-----------------------------------------------
// fn put_request(homepath: &PathBuf,filename: &str) -> &str {}



//-----------------------------------------------
//Funtion To Process GET API Call
//Retrieve homedir as homepath and
//Request URI as resources and
//Stream as TCPStream
//-----------------------------------------------
fn get_api_testsse(mut stream: TcpStream,homepath: &PathBuf,filename: &str) {
    if print_debug() { println!("DEBUG 0022 : \"TESTSSE\" GET API CALLED"); }
    let mut response = String::new();
    let mut contents = String::new();
    let mut buffer = [0; BUFFER];

    //Prepare HTTP Header
    let status_line = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\n\r\n";

    //stream.shutdown(Shutdown::Read).expect("shutdown call failed");
    loop {
        //contents = "data: {\"data\": \"test content\"}\r\n\r\n".to_string();
        contents = "data: <img src=\"image/test.jpg\" alt=\"sample\">\r\n\r\n".to_string();

        response = format!("{}{}", status_line, contents);

        if print_debug() { println!("DEBUG 0024 : Response : {:?}",response); }
        
        //stream.write(response.as_bytes()).unwrap();
        //escaping from possible write on rst socket
        //exit thread before panic
        match stream.write(response.as_bytes()) {
            Ok(n) => stream.flush().unwrap(),
            Err(err) => break,
        }

        thread::sleep(time::Duration::from_millis(10000));
    }
}


fn get_api_lastresort(mut stream: TcpStream,homepath: &PathBuf,filename: &str) {
    if print_debug() { println!("DEBUG 0021 : GET API CALL GOES TO LAST RESORT"); }
    let mut _response = String::new();
    _response = format!("NOT IMPLEMENTED");
    stream.write(_response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

//-----------------------------------------------
//Funtion To Process POST API Call
//Retrieve homedir as homepath and
//Request URI as resources and
//Stream as TCPStream
//-----------------------------------------------
fn post_api_filescan(mut _stream: TcpStream,homepath: &PathBuf,filename: &str,data: &Vec<u8>) {
    let mut _response = String::new();
    if print_debug() { println!("DEBUG 0049 retrieved data: {}", String::from_utf8_lossy(&data)); }

    //write to file
    let my_uuid = Uuid::new_v4();
    
    //let mut wfile = std::fs::File::create("test").unwrap();
    let mut wfile = std::fs::File::create(my_uuid.to_string()).unwrap();
    wfile.write(&data[0..]);

    let scan = Command::new("sh")
            .arg("-c")
            .arg(format!("clamscan {}",my_uuid.to_string()))
            .output()
            .expect("failed to execute process");
    
    if print_debug() { println!("DEBUG 0050 scan result: {}", std::str::from_utf8(&scan.stdout).unwrap()); }

    //get file sha256
    let sha256 = Command::new("sh")
            .arg("-c")
            .arg(format!("sha256sum -b {}",my_uuid.to_string()))
            .output()
            .expect("failed to execute process");
    

    //response
    _response = format!("HTTP/1.1 200 OK\r\n\r\n");
    _stream.write(_response.as_bytes());

    _response = format!("{}\r\n",std::str::from_utf8(&sha256.stdout).unwrap());
    _stream.write(_response.as_bytes());

    _response = format!("{}",std::str::from_utf8(&scan.stdout).unwrap());
    _stream.write(_response.as_bytes());
    
    
    let fileremove = Command::new("sh")
            .arg("-c")
            .arg(format!("rm {}",my_uuid.to_string()))
            .output()
            .expect("failed to execute process");
    
}

fn post_api_mu(mut _stream: TcpStream,homepath: &PathBuf,filename: &str,data: &Vec<u8>) {
    let mut _response = String::new();
    if print_debug() { println!("DEBUG 0057 retrieved data: {}", String::from_utf8_lossy(&data)); }

    //write to file
    let my_uuid = Uuid::new_v4();

    let folder = std::fs::create_dir(my_uuid.to_string());
    let mut filepath = format!("{}/{}",my_uuid.to_string(),my_uuid.to_string());

    let mut wfile = std::fs::File::create(&filepath).unwrap();
    wfile.write(&data[0..]);
    
    let mut runfunction = Command::new("sh")
            .arg("-c")
            .arg(format!("python3 {}",filepath))
            //.output()
            //.expect("failed to execute process");
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

    let mut timeout_flag = false;
    let secs = Duration::from_secs(MU_TIMEOUT);
    let _status_code = match runfunction.wait_timeout(secs).unwrap() {
        Some(status) => {
            status.code() 
        },
        None => {
            timeout_flag = true;
            runfunction.kill().unwrap();
            runfunction.wait().unwrap().code()
        }
    };
    
    _response = format!("HTTP/1.1 200 OK\r\n\r\n");
    _stream.write(_response.as_bytes());

    if timeout_flag {
        _response = format!("Program timeout reached");
        _stream.write(_response.as_bytes());
    }
    /*
    _response = format!("{}",std::str::from_utf8(&runfunction.stdout).unwrap());
    _stream.write(_response.as_bytes());
    _response = format!("{}",std::str::from_utf8(&runfunction.stderr).unwrap());
    _stream.write(_response.as_bytes());
    */

    let mut s = String::new();
    let mut statusflag = false;
    runfunction.stdout.unwrap().read_to_string(&mut s).unwrap();
    for (num, line) in s.split("\n").enumerate() {
        println!("{}: {}", num, line);
        _stream.write(line.as_bytes());
    }


    //check stdout
    //println!("{}", _status_code.unwrap().to_string());
    if _status_code.unwrap() == 1 {
        runfunction.stderr.unwrap().read_to_string(&mut s).unwrap();
        for (num, line) in s.split("\n").enumerate() {
            println!("{}: {}", num, line);
            _stream.write(line.as_bytes());
        }
    }


    /*
    let fileremove = Command::new("sh")
            .arg("-c")
            .arg(format!("rm {}",my_uuid.to_string()))
            .output()
            .expect("failed to execute process");
    */
    let folderremove = Command::new("sh")
            .arg("-c")
            .arg(format!("rm -rf {}",my_uuid.to_string()))
            .output()
            .expect("failed to execute process");

}

fn post_api_nu(mut _stream: TcpStream,homepath: &PathBuf,filename: &str,data: &Vec<u8>) {
    let mut _response = String::new();
    if print_debug() { println!("DEBUG 0058 : retrieved data {}", String::from_utf8_lossy(&data)); }

    let mut containerid = String::new();

    let mut base_dockerfile = homepath.clone();
    base_dockerfile.push("base_dockerfile");
    //html folder

    //write to file
    let my_uuid = Uuid::new_v4();

    let folder = std::fs::create_dir(my_uuid.to_string());
    let mut folderpath = format!("{}",my_uuid.to_string());
    let mut filepath = format!("{}/{}",my_uuid.to_string(),my_uuid.to_string());

    let mut wfile = std::fs::File::create(&filepath).unwrap();
    wfile.write(&data[0..]);

    //TODO prepare a container
    let runfunction = Command::new("sh")
            .arg("-c")
            .arg(format!("cp {} {}",base_dockerfile.to_string_lossy(),my_uuid.to_string()))
            .output()
            .expect("failed to execute process");
    
    //1.build dockerfile
    let builddockerfilefunction = Command::new("sh")
            .arg("-c")
            .arg(format!("cat {}/base_dockerfile > {}/dockerfile",my_uuid.to_string(),my_uuid.to_string()))
            .output()
            .expect("failed to execute process");

    //2.base container is ryosuha/nu-system:1.0
    //3.copy retrieved python code to container
    let builddockerfilefunction = Command::new("sh")
            .arg("-c")
            .arg(format!("echo \"\nCOPY {} /\n\" >> {}/dockerfile",my_uuid.to_string(),my_uuid.to_string()))
            .output()
            .expect("failed to execute process");

    //4.entrypoint to copied python code
    
    let builddockerfilefunction = Command::new("sh")
            .arg("-c")
            //.arg(format!("echo \"\nENTRYPOINT /root/{}\n\" >> {}/dockerfile",my_uuid.to_string(),my_uuid.to_string()))
            .arg(format!("echo \'\nCMD [\"python3\",\"/{}\"]\n\' >> {}/dockerfile",my_uuid.to_string(),my_uuid.to_string()))
            .output()
            .expect("failed to execute process");
    
    //5.build container
    let builddockerfilefunction = Command::new("sh")
            .arg("-c")
            .arg(format!("docker build -f {}/dockerfile .",my_uuid.to_string()))
            .output()
            .expect("failed to execute process");
    
    //6.retrieve container image id
    let parsedline: Vec<&str> = str::from_utf8(&builddockerfilefunction.stdout).unwrap().split("\n").collect();
    for _i in parsedline.iter() {
        if GETCONTAINER_IMAGE_ID.is_match(_i){
            let parsed: Vec<&str> = _i.split_whitespace().collect();
            containerid = remove_all_whitespace(&parsed[2].to_string());
            if print_debug() { println!("DEBUG 0059 : Created image ID is {}",containerid); }
        }
    }

    //TODOpush container to private registry
    //tag
    let pushcontainerimage = Command::new("sh")
            .arg("-c")
            .arg(format!("docker tag {} {}/{}",containerid,NU_SYSTEM_REGISTRY.to_string(),containerid))
            .output()
            .expect("failed to execute process");
    if print_debug() { println!("DEBUG 0064 : docker tag {} {}/{}",containerid,NU_SYSTEM_REGISTRY.to_string(),containerid); }
    _response = format!("{}",std::str::from_utf8(&pushcontainerimage.stderr).unwrap());
    if print_debug() { println!("DEBUG 0065 : docker tag error {}",_response); }
    //push
    let pushcontainerimage = Command::new("sh")
            .arg("-c")
            .arg(format!("docker push {}/{}",NU_SYSTEM_REGISTRY.to_string(),containerid))
            .output()
            .expect("failed to execute process");

    //6.make knative serving manifest
    let mut base_servingfile = homepath.clone();
    base_servingfile.push("base_serving.yaml");

    let mut servingyaml = File::open(base_servingfile).unwrap();
    let mut servingyaml_file = String::new();
    servingyaml.read_to_string(&mut servingyaml_file);
    if print_debug() { println!("DEBUG 0060 : Serving yaml contents {}",servingyaml_file); }

    let parsed_servingyaml_file: Vec<&str> = servingyaml_file.split("\n").collect();
    let mut line_buffer = String::new();
    let mut servingyamlpath = format!("{}/{}.yaml",my_uuid.to_string(),my_uuid.to_string());
    let mut writed_servingyaml_file = std::fs::File::create(servingyamlpath).unwrap();
    let generated_dnsname = newdnsname();

    for _j in parsed_servingyaml_file.iter() {
        println!("{}",_j);
        
        if MAKING_SERVING_MANIFEST_MATCHCNTID.is_match(_j) {
            line_buffer = _j.to_string();
            let result = MAKING_SERVING_MANIFEST_MATCHCNTID.replace(&line_buffer,&containerid);
            if print_debug() { println!("DEBUG 0062 : Parsed serving yaml {}",result); }
            //writed_servingyaml_file.write(&result.as_bytes()); same as below
            writeln!(writed_servingyaml_file,"{}",result);
            continue;
        }

        if MAKING_SERVING_MANIFEST_MATCHUUID.is_match(_j) {
            line_buffer = _j.to_string();
            //let result = MAKING_SERVING_MANIFEST_MATCHUUID.replace(&line_buffer,replace_all_hyphen(&my_uuid.to_string()));
            let result = MAKING_SERVING_MANIFEST_MATCHUUID.replace(&line_buffer,&generated_dnsname);
            if print_debug() { println!("DEBUG 0063 : Parsed serving yaml {}",result); }
            writeln!(writed_servingyaml_file,"{}",result);
            continue;
        }
        
        writeln!(writed_servingyaml_file,"{}",_j);
        if print_debug() { println!("DEBUG 0061 : Parsed serving yaml {}",_j); }
    }

    //7.apply serving manifest
    let applyservingmanifestfunction = Command::new("sh")
            .arg("-c")
            .arg(format!("kubectl apply -f {}/{}.yaml",my_uuid.to_string(),my_uuid.to_string()))
            .output()
            .expect("failed to execute process");


    //8.respond webhook url


    _response = format!("HTTP/1.1 200 OK\r\n\r\n");
    _stream.write(_response.as_bytes());

    _response = format!("{}",std::str::from_utf8(&builddockerfilefunction.stdout).unwrap());
    _stream.write(_response.as_bytes());

    _response = format!("created service on fqdn : http://{}.{}",&generated_dnsname,NU_SYSTEM_DOMAIN.to_string());
    _stream.write(_response.as_bytes());
    

    //TODO DELETION OF SERVICE 
    //SHOULD BE ON DELETE API METHOD
    /*
    let fileremove = Command::new("sh")
            .arg("-c")
            .arg(format!("rm {}",my_uuid.to_string()))
            .output()
            .expect("failed to execute process");
    */

    /*
    let folderremove = Command::new("sh")
            .arg("-c")
            .arg(format!("rm -rf {}",my_uuid.to_string()))
            .output()
            .expect("failed to execute process");
    */
}

//-----------------------------------------------
//Funtion To Process UNKNOWN API Call
//Retrieve homedir as homepath and
//Request URI as resources and
//Stream as TCPStream
//Shutdown TCP Connection immediately
//-----------------------------------------------
fn api_lastresort(mut _stream: TcpStream,homepath: &PathBuf,filename: &str) {
    _stream.shutdown(Shutdown::Both).expect("shutdown call failed");
    if print_debug() { println!("DEBUG 0017 : API CALL GOES TO LAST RESORT"); }
}


//-----------END OF REQUEST ROUTING FUNCTION----------//


//--------------BEGIN OF GENERIC FUNCTION-------------//
fn file_check(filepath: &PathBuf) -> bool {
    if print_debug() { println!("DEBUG 0025 : is file exist? {}", Path::new(&filepath).exists()); }
    Path::new(&filepath).exists()
}

fn remove_first_slash(target: &str) -> &str {
    let mut result = target.chars();
    result.next();
    result.as_str()
}

fn remove_all_whitespace(target: &String) -> String {
    let mut result = target;
    result.replace(" ", "")
}

fn replace_all_hyphen(target: &String) -> String {
    let mut result = target;
    result.replace("-", "_")
}

fn is_api(requests: &str) -> bool {
    let parsedrequests: Vec<&str> = requests.split("/").collect();
    for _i in parsedrequests.iter() {
        if print_debug() { println!("DEBUG 0012 : {}",_i); }
    }
    if parsedrequests[0] == "api" {
        true
    }
    else {
        false
    }
}

fn change_debug(flag: bool) {
    if flag { *DEBUG_MODE.lock().unwrap() = true; }
    else { *DEBUG_MODE.lock().unwrap() = false; }
}

fn change_debugdata(flag: bool) {
    if flag { *DEBUGDATA_MODE.lock().unwrap() = true; }
    else { *DEBUGDATA_MODE.lock().unwrap() = false; }
}

fn print_debug() -> bool{
    //println!("DEBUG : {}", *DEBUG_MODE);   //DEBUGing a DEBUG function
    //if *DEBUG_MODE { true }
    if *DEBUG_MODE.lock().unwrap() { true }
    else { false }
}

fn print_debugdata() -> bool{
    //println!("DEBUG : {}", *DEBUG_MODE);   //DEBUGing a DEBUG function
    //if *DEBUG_MODE { true }
    if *DEBUGDATA_MODE.lock().unwrap() { true }
    else { false }
}

//---------------END OF GENERIC FUNCTION--------------//

//---------BEGIN OF FEAtURE SPECIFIC FUNCTION---------//
fn newdnsname() -> String {
    let mut rng = rand::thread_rng();
    let letter1: char = rng.gen_range(b'a'..=b'z') as char;
    let letter2: char = rng.gen_range(b'a'..=b'z') as char;
    let letter3: char = rng.gen_range(b'a'..=b'z') as char;
    let letter4: char = rng.gen_range(b'a'..=b'z') as char;
    let number: u32 = rng.gen_range(0..9999);
    format!("{}{}{}{}{:04}",letter1,letter2,letter3,letter4,number)
    
}

//----------END OF FEAtURE SPECIFIC FUNCTION----------//