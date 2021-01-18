// Suppress warning here is because it is mistakenly treat the code as dead code when running unit tests.
#![allow(dead_code)]

use crate::util::{
    deploy_builtin_contract, deploy_contract, hex_to_byte32, hex_to_bytes, 
    mock_cell_with_outpoint, mock_dep, mock_input, mock_script,mock_output,
};
use ckb_testtool::{
    builtin::ALWAYS_SUCCESS,
    context::{random_out_point, Context},
};
use ckb_tool::ckb_types::{
    bytes::Bytes,
    core::ScriptHashType,
    core::TransactionBuilder,
    core::TransactionView,
    packed::*,
    prelude::{Builder, Entity, Pack},
};
use lazy_static::lazy_static;
use regex::{Regex,Captures};

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Read;

use serde::{Deserialize, Serialize};

lazy_static! {
    static ref VARIABLE_REG: Regex = Regex::new(r"\{\{(\w+)\}\}").unwrap();
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonContract {
    pub name: String,
    pub mode: String,
    pub file: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonScript {
    #[serde(default)]
    pub code_hash: String,
    #[serde(default)]
    pub args: String,
    #[serde(default)]
    pub hash_type: String,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonCell {
    pub capacity: u64,
    pub lock_script: JsonScript,
    pub type_script: JsonScript,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    pub out_point: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HopeResult {
    #[serde(default)]
    pub error_type: String,
    #[serde(default)]
    pub error_number: i8,
    #[serde(default)]
    pub cell_index: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonData {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub contracts: Vec<JsonContract>,
    #[serde(default)]
    pub script_hash: HashMap<String, JsonScript>,
    #[serde(default)]
    pub cell_deps: Vec<JsonCell>,
    pub inputs: Vec<JsonCell>,
    pub outputs: Vec<JsonCell>,
    #[serde(default)]
    pub witnesses: Vec<String>,
    pub hope_result: HopeResult,
}

pub struct TemplateParser<'a> {
    context: &'a mut Context,
    pub data: JsonData,
    contracts: HashMap<String, Byte32>,
    script_hash: HashMap<String, Byte32>,
    deps: Vec<CellDep>,
    inputs: Vec<CellInput>,
    outputs: Vec<CellOutput>,
    outputs_data: Vec<Bytes>,
    witnesses: Vec<Bytes>,
}

impl std::fmt::Debug for TemplateParser<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TemplateParser")
            .field("contracts", &self.contracts)
            .field("deps", &self.deps)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("outputs_data", &self.outputs_data)
            .finish()
    }
}

impl<'a> TemplateParser<'a> {
    pub fn new(context: &'a mut Context, raw_json: &str) -> Result<Self, Box<dyn Error>> {
        let data: JsonData = serde_json::from_str(raw_json)?;

        Ok(TemplateParser {
            context,
            data,
            contracts: HashMap::new(),
            script_hash: HashMap::new(),
            deps: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            outputs_data: Vec::new(),
            witnesses: Vec::new(),
        })
    }

    pub fn from_file(context: &'a mut Context, file_name: String) -> Result<Self, Box<dyn Error>> {
        let mut s = String::new();
        File::open(file_name)?.read_to_string(&mut s)?;
        let data: JsonData = serde_json::from_str(&s[..])?;

        Ok(TemplateParser {
            context,
            data,
            contracts: HashMap::new(),
            script_hash: HashMap::new(),
            deps: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            outputs_data: Vec::new(),
            witnesses: Vec::new(),
        })
    }

    pub fn parse(&mut self) -> () {
        if let Err(e) = self.try_parse() {
            panic!(format!("{}", e.to_string()));
        }
    }

    pub fn try_parse(&mut self) -> Result<(), Box<dyn Error>> {
        self.parse_contract()?;
        self.parse_script_hash()?;
        self.parse_cell("dep")?;
        self.parse_cell("input")?;
        self.outputs_data = self.parse_cell("output").expect("error outputs");
        self.witnesses = self.parse_witnesses();

        Ok(())
    }

    pub fn set_outputs_data(&mut self, i: usize, data: Bytes) {
        self.outputs_data[i] = data;

        eprintln!("Set self.outputs_data = {:#?}", self.outputs_data);
    }

    pub fn build_tx(&mut self) -> TransactionView {
        TransactionBuilder::default()
            .cell_deps(self.deps.clone())
            .inputs(self.inputs.clone())
            .outputs(self.outputs.clone())
            .outputs_data(self.outputs_data.pack())
            .witnesses(self.witnesses.pack())
            .build()
    }

    
    fn parse_contract(&mut self) -> Result<(), Box<dyn Error>> {
        let always_success_out_point = self.context.deploy_cell(ALWAYS_SUCCESS.clone());
        let (script, cell_dep) =
            mock_script(self.context, always_success_out_point, Bytes::default());
        self.deps.push(cell_dep.clone());
        self.contracts
            .insert("always_success".to_string(), script.code_hash());

        for item in self.data.contracts.clone() {
            match &item.mode[..] {
                "deployed" => {
                    let out_point = deploy_builtin_contract(self.context, &item.file[..]);
                    let (script, cell_dep) = mock_script(self.context, out_point, Bytes::default());

                    self.deps.push(cell_dep.clone());
                    self.contracts
                        .insert(item.name, script.code_hash());
                }
                _ => {
                    let out_point = deploy_contract(self.context, &item.file[..]);
                    let (script, cell_dep) = mock_script(self.context, out_point, Bytes::default());

                    self.deps.push(cell_dep.clone());
                    self.contracts
                        .insert(item.name, script.code_hash());
                }
            }
        }

        // eprintln!("Parse self.contracts = {:#?}", self.contracts);
        // eprintln!("Parse self.contract = {:#?}", self.deps);
        Ok(())
    }
    
    fn parse_script_hash(&mut self) -> Result<(), Box<dyn Error>> {
        for (name,item) in self.data.script_hash.clone() {
            match self.parse_script(item)?{
                Some(script)=>{
                    self.script_hash.insert(name, script.calc_script_hash())
                },
                _ =>panic!("error script hash:{}",name)
            };
        }

        Ok(())
    }

    fn parse_cell(
        &mut self,
        mode: &str,
        // cells: Vec<JsonCell>,
    ) -> Result<Vec<Bytes>, Box<dyn Error>> {
        let mut datas:Vec<Bytes> = Vec::new();
        let cells = match mode{
            "dep" => self.data.cell_deps.clone(),
            "input" =>  self.data.inputs.clone(),
            "output" =>  self.data.outputs.clone(),
            _ => panic!("unknow mode")
        };

        let mut i:u64 = 0;
        for it in cells{
            // parse lock script and type script of cell
            let lock_script = self.parse_script(it.lock_script).expect("error lock script");
            let type_script = self.parse_script(it.type_script).expect("error type script");
    
            let data_str = VARIABLE_REG.replace_all(&it.data[..], |caps: &Captures| {
                let script_name = caps.get(1).map(|m| m.as_str()).unwrap();
                let hash = match self.script_hash.get(script_name) {
                    Some(h) => h.clone(),
                    None => match self.contracts.get(script_name) {
                        Some(h) => h.clone(),
                        None => panic!("not found script:{}", script_name)
                    }
                };
                format!("{:x}",hash)
            }).to_string();
            // parse data of cell
            let data = hex_to_bytes(&data_str[..]).expect(format!("error data:{}", data_str).as_str());
    
            let outpoint;
            if it.out_point != "" {
                let hex_str = hex_to_bytes(&it.out_point[..])?;
                outpoint = OutPoint::new_unchecked(hex_str);
            } else {
                outpoint = random_out_point();
            }
            datas.push(data.clone());
           
            i = i+1;
            match mode{
                "dep" => {
                    mock_cell_with_outpoint(
                        self.context,
                        outpoint.clone(),
                        it.capacity,
                        lock_script.unwrap(),
                        type_script,
                        Some(data),
                    );
                    self.deps.push(mock_dep(outpoint))
                },
                "input" => {
                    mock_cell_with_outpoint(
                        self.context,
                        outpoint.clone(),
                        it.capacity,
                        lock_script.unwrap(),
                        type_script,
                        Some(data),
                    );
                    self.inputs.push(mock_input(outpoint,Some(i)))
                },
                "output" => self.outputs.push(mock_output(it.capacity,lock_script.unwrap(),type_script)),
                _ => panic!("unknow mode")
            }
        }

        Ok(datas)
    }

    fn parse_script(&self, script_info: JsonScript) -> Result<Option<Script>, Box<dyn Error>> {
        if script_info.code_hash == "" {
            return Ok(None);
        }

        let hash_str = VARIABLE_REG.replace_all(&script_info.code_hash[..], |caps: &Captures| {
            let script_name = caps.get(1).map(|m| m.as_str()).unwrap();
            let hash = match self.contracts.get(script_name) {
                Some(h) => h.clone(),
                None => panic!("not found script:{}", script_name)
            };
            format!("{:x}",hash)
        }).to_string();
        let code_hash = hex_to_byte32(&hash_str[..]).expect(format!("error code_hash:{}", hash_str).as_str());
        
        let args_str = VARIABLE_REG.replace_all(&script_info.args[..], |caps: &Captures| {
            let script_name = caps.get(1).map(|m| m.as_str()).unwrap();
            let hash = match self.script_hash.get(script_name) {
                Some(h) => h.clone(),
                None => match self.contracts.get(script_name) {
                    Some(h) => h.clone(),
                    None => panic!("not found script:{}", script_name)
                }
            };
            format!("{:x}",hash)
        }).to_string();
        let args = hex_to_bytes(&args_str[..]).expect(format!("error args:{}", args_str).as_str());
       
        let hash_type = match &script_info.hash_type[..] {
            "type" => ScriptHashType::Type,
            _ => ScriptHashType::Data,
        };
        let script = Some(Script::new_builder()
                .code_hash(code_hash.clone())
                .hash_type(hash_type.into())
                .args(args.pack())
                .build());
        
        Ok(script)
    }

    
    fn parse_witnesses(
        &mut self,
        // cells: Vec<JsonCell>,
    ) -> Vec<Bytes> {
        let mut datas:Vec<Bytes> = Vec::new();

        for it in self.data.witnesses.clone(){
            let args_str = VARIABLE_REG.replace_all(&it[..], |caps: &Captures| {
                let script_name = caps.get(1).map(|m| m.as_str()).unwrap();
                let hash = match self.script_hash.get(script_name) {
                    Some(h) => h.clone(),
                    None => match self.contracts.get(script_name) {
                        Some(h) => h.clone(),
                        None => panic!("not found script:{}", script_name)
                    }
                };
                format!("{:x}",hash)
            }).to_string();
            let data = hex_to_bytes(&args_str[..]).expect(format!("error witnesses:{}", args_str).as_str());
            datas.push(data.clone());
        }

        datas
    }
}
