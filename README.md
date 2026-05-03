> [!IMPORTANT]  
> Deprecated version

<img src="assets/hero.png" alt="httui" style="width: 100%" />

# Overview

**httui** is an alternative to the `Postman`/`Insomnia` application designed to run in the terminal. It's a TUI (Text User Interface) based application that simplifies the process of creating, executing, and managing HTTP requests directly in the terminal. Additionally, it has native integration with VIM, making request editing even more convenient for users familiar with this text editor.

https://github.com/gandarfh/httui/assets/57275106/d6865bd4-036d-4a2c-916d-7f89089b0749

# Features

## 1. Variables in Multiple Environments

**httui** allows you to define variables in different environments, making it easy to parameterize your requests. This is especially useful when you need to switch between development, testing and production environments.

## 2. Easy Environment Creation and Switching

Creating and switching between environments is a straightforward task in **httui**. You can easily define environments and select them according to your needs.

## 3. Access to Variables Using Patterns

**httui** enables you to access variables using patterns, making variable substitution in requests more efficient and flexible.

## 4. Request Grouping

You can organize your requests into logical groups, making it easier to organize and execute multiple related requests together.

## 5. Reuse of Parameters or Headers in Groups

**httui** allows you to reuse parameters or headers across all requests within a group, simplifying the configuration of similar requests.

## 6. Using the Result of One Request in Another

You can use the result of one request as a value in another, saving time and making your requests more dynamic. You can also store results in shared variables for later access.

## 7. Easy Request Execution

**httui** makes request execution simple and straightforward. You can execute your requests with just a few clicks, making the testing and debugging process fast and efficient.

# Keymaps

| **Key**       | **Action**                               |
| ------------- | ---------------------------------------- |
| `/`           | Filter requests by name                  |
| `ctrl` + `s`  | Change environments                      |
| `shift` + `s` | Create a new environments                |
| `ctrl` + `e`  | Open variables for edit                  |
| `a` or `c`    | Create request or request group          |
| `d`           | Delete request or request group          |
| `shift` + `r` | Edit request or request group            |
| `e`           | Execute request                          |
| `h` or `l`    | Navigate betwen groups                   |
| `j` or `k`    | Move between requests or requests groups |

# Tech Stack

- [Go](https://go.dev/)
- [Sqlite](https://www.sqlite.org/index.html)
- [Bubbletea](https://github.com/charmbracelet/bubbletea)
- [Gorm](https://gorm.io/gorm)

# Roadmap

- [ ] Import requests from a Swagger file.
- [ ] Sync database on S3.
- [ ] Support more text editors (vim, vscode, nano, emacs)

## Contact

Twitter - [@gandarfh](https://twitter.com/gandarfh)
