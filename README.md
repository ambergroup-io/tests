# tests

## 功能说明

这是一个nervos的测试框架，可以直接替换capsule项目的tests。

它会自动加载templates中的所有json文件，执行测试。

每个json文件对应一个ckb交易，也对应一个测试用例。

这个只需要编译一次，后续只需要添加相应的json文件就可以。

普通测试场景只需要配置相应的json文件，无需编写测试代码。

## 配置说明

1. templates文件夹下的json文件，每个文件对应一个测试
2. 可以在templates文件夹下创建子文件夹，将不同的测试用例放在不同的文件夹下
3. 配置说明：(new_config.json)
   1. "contracts":列出需要加载的脚本
      1. name：脚本的名字，将在cell.script.code_hash使用；lock_script或type_script
      2. mode：
         1. 当前默认都是default，它决定从哪个文件夹下加载script文件。
         2. deployed：将从deployed-scripts文件夹下加载文件
      3. file：脚本的名字，默认将从build/debug/中加载该脚本文件
      4. 系统自动添加了名字为always_success的脚本，永远返回成功，可以作为通用的lock
   2. "script_hash"：列出需要动态计算的script_hash
      1. key为名字，如"index_hash"，将在cell.script.args使用
      2. code_hash、hash_type、args：需要计算的3个参数，其中code_hash可以引用前面的contracts中的脚本
```json
"index_hash": {
   "args": "0xf1d1b3ddcca92b1e49783769e9bf606112b3f8cf36b96cac05bf44edcf5377e600000000",
   "code_hash": "{{index}}",
   "hash_type": ""
}
```
   3. "cell_deps": 需要依赖的cell列表，内容都是cell格式，参考后面说明
   4. "inputs"：输入的cell列表
   5. "outputs"：输入的cell列表
   6. cell格式：
      1. capacity：ckb的capacity
      2. lock_script：lock脚本，如果没有要求，"code_hash"设置为"{{always_success}}"就可以
      3. type_script：根据自己的业务场景，设置参数
         1. "code_hash": "{{index}}",引用项目中的index脚本，需要在前面的contracts里配置，也可以直接填写具体的hex字符串
         2. "args": "1234{{index_hash}}abcd{{index2_hash}}",可以填写具体的值(hex)，也可以填写前面的script_hash或contracts;
         3. hash_type:默认为空(data)，可以设置为data或type
      4. data: cell的data
      5. out_point：可以指定；如果为"",将使用随机的值。该值在output中不生效。
```json
{
   "capacity": 1000,
   "lock_script": {
         "args": "",
         "code_hash": "{{always_success}}",
         "hash_type": ""
   },
   "type_script": {
         "args": "0xf1d1b3ddcca92b1e49783769e9bf606112b3f8cf36b96cac05bf44edcf5377e600000000",
         "code_hash": "{{index}}",
         "hash_type": ""
   },
   "data": "0x01000000",
   "out_point": "0xf2d1b3ddcca92b1e49783769e9bf606112b3f8cf36b96cac05bf44edcf5377e600000000"
}
```
   7. witnesses: string数组，默认存放签名信息，可以为空
      1. 要求是hex字符串
      2. 与script的args一样，也可以动态引用script_hash或contracts
   8. hope_result是预期的测试结果。
      1. error_type：
         1. 为""，表示成功
         2. 为“input”表示input.type_script错误
         3. 为output表示output.type_script错误
         4. 为lock表示input.lock_script错误
      2. cell_index表示错误的cell序号，默认为0
      3. error_number表示脚本返回的错误码
      4. 如果是错误用例，建议添加description，说明错误场景
1. test_all默认只有一个线程，遍历文件夹，加载测试。
   1. 如果中间有用例失败，则中断退出。
   2. 默认只遍历json文件，在src/tests.rs:72限制：entry.path().extension().unwrap() != "json"
2. 可以自己添加对应的test function，使其可以并行运行。
   1. 具体参考src/tests.rs:90，new_test!(1, "./templates/first_index.json");每一行对应一个测试用例
   2. 建议使用其他后缀，json后缀会自动加载，非json后缀可以手动添加
