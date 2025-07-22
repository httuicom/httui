package tui

import (
	"fmt"
	"os"
	"path/filepath"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/internal/requests"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/config"
	"gorm.io/gorm"
)

var (
	program  tea.Program
	database *gorm.DB
)

func init() {
	storage, err := storage.SqliteConnection()
	if err != nil {
		fmt.Println("Error running program:", err)
		os.Exit(1)
	}

	database = storage
}

func App() {
	configDir, _ := os.UserHomeDir()
	path := filepath.Join(configDir, config.APP_DIR, "debug.log")

	f, err := tea.LogToFile(path, "debug")
	if err != nil {
		fmt.Println("fatal:", err)
		os.Exit(1)
	}
	defer f.Close()

	m := requests.New(database)

	program = *tea.NewProgram(m, tea.WithAltScreen())
	if _, err := program.Run(); err != nil {
		fmt.Println("Error running program:", err)
		os.Exit(1)
	}
}
