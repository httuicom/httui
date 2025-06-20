package cmd

import (
	"log"

	"github.com/gandarfh/httui/internal/tui"
	"github.com/gandarfh/httui/pkg/config"
	"github.com/spf13/cobra"
)

var (
	rootCmd = &cobra.Command{
		Version: "v0.0.1",
		Use:     "httui",
		Long:    "Welcome to httui, your terminal-based TUI application for managing HTTP requests efficiently. This guide will help you familiarize yourself with the basic operations and navigation within httui. ",
		Example: "httui",
		RunE: func(cmd *cobra.Command, args []string) error {
			tui.App()
			return nil
		},
	}
)

func Execute() error {
	return rootCmd.Execute()
}

func init() {
	err := config.ParseConfig()
	if err != nil {
		log.Fatal(err)
	}

	rootCmd.AddCommand(testCmd())
}
