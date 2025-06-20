package config

import (
	"fmt"
	"path"
)

// configError represents an error that occurred while parsing the config file.
type configError struct {
	configDir string
	parser    ConfigParser
	err       error
}

// Error returns the error message for when a config file is not found.
func (e configError) Error() string {
	return fmt.Sprintf(
		`Couldn't find a httui.yml configuration file.
Create one under: %s
Example of a config.yml file:
%s
press q to exit.
Original error: %v`,
		path.Join(e.configDir, APP_DIR, CONFIG_FILE_NAME),
		e.parser.getDefaultConfigYamlContents(),
		e.err,
	)
}

// parsingError represents an error that occurred while parsing the config file.
type parsingError struct {
	err error
}

// Error represents an error that occurred while parsing the config file.
func (e parsingError) Error() string {
	return fmt.Sprintf("failed parsing config.yml: %v", e.err)
}
