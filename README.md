# pgsubset

pgsubset is a tool to simplify the process of exporting and importing a part of a Postgres database.

it calculates all the objects needed for a target table to mantain referential integrity and it uses pg's `COPY` instructions to efficiently export them as csv files.

it also include an import command which can be used to re-import those data into another database with the same schema.

```
pgsubset 0.1.0
Utility to export a referentially intact subset of a Postgres Database and re-import to another
location.

USAGE:
    pgsubset --config <CONFIG> --mode <MODE>

OPTIONS:
    -c, --config <CONFIG>
    -h, --help               Print help information
    -m, --mode <MODE>        [possible values: export, import]
    -V, --version            Print version information
```

## Installation

``` sh
# install rustup
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# install stable toolchain
$ rustup toolchain install stable
# set default toolchain
$ rustup default stable
# add cargo bin dir to PATH
$ echo "export PATH=$PATH:~/.cargo/bin" >> ~/.bashrc
# build and install pgsubset
$ cargo install --path .
```


## Usage

in order to run the tool in both export and import mode a config file in TOML format must be supplied with the `--config` argument.

the required minimal config should include:

  * `database_url` -> target database for import/export
  * `target_table` -> table used as entrypoint for the subset
  * `target_dir` -> directory used for storing exported data or reading data to be imported

a complete example can be found [here](#config-file).

### Export mode

``` sh
$ pgsubset -c subset.toml --mode export
${target_dir}/03-table_3.csv writed
${target_dir}/02-table_2.csv writed
${target_dir}/01-table_1.csv writed
```

### Import mode

``` sh
$ pgsubset -c subset.toml --mode import
imported ${target_dir}/00-table_1.csv to table_1
imported ${target_dir}/01-table_2.csv to table_2
imported ${target_dir}/02-table_3.csv to table_3
```

### Data Manipulation

data can be modified on the fly when exporting.

this can be useful in order to hide senstive informations or clear fields that are not needed.

data manipulation can be configured with `transforms` key as:

``` toml
[transforms]
# <table_1>.<field_1> = "<transform>"
# <table_1>.<field.2> = "<transform>"
```

currently supported transformations are:
  * `clear_field`
  * `first_name_en`
  * `last_name_en`
  * `username_en`
  * `email_en`
  * `django_garbage_password`

### Many To Many

many-to-many relationships should be specified using `[[m2m_tables]]` config keys with:

 * `name` -> the junction table name
 * `source` -> the table which is already within dependency graph

otherwise the relationship won't be included in the dump.

for example

```
         BC
       /    \
A -> B       C
```

can be specified as:

``` toml
target_table = "A"

[[m2m_tables]]
name="BC"
source = "B"
```

## Config file

``` toml
database_url="<TARGET_DATABASE_URL>"
target_table = "<TARGET_TABLE_FOR_EXPORT>"
target_dir = "<EXPORT_PATH>"

[transforms]
# <table>".<field> = "<transform>"

[[m2m_tables]]
name = "<ONE_JUNCTION_TABLE>"
source = "<ITS_SOURCE_TABLE>"
```

## Credits

Many thanks to [@dodomorandi](https://github.com/dodomorandi) for the review effort and for his very valuable advice!
