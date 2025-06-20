package tui

import (
	"fmt"
	"os"
	"path/filepath"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/pkg/config"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/internal/requests"
)

func init() {
	if err := storage.SqliteConnection(); err != nil {
		fmt.Println("Error running program:", err)
		os.Exit(1)
	}

}

var (
	program tea.Program
)

func App() {
	configDir, _ := os.UserHomeDir()
	path := filepath.Join(configDir, config.APP_DIR, "debug.log")

	f, err := tea.LogToFile(path, "debug")
	if err != nil {
		fmt.Println("fatal:", err)
		os.Exit(1)
	}
	defer f.Close()

	m := requests.New()

	program = *tea.NewProgram(m, tea.WithAltScreen())

	if _, err := program.Run(); err != nil {
		fmt.Println("Error running program:", err)
		os.Exit(1)
	}
}
