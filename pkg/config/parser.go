package config

import (
	"os"
	"path/filepath"

	"gopkg.in/yaml.v3"
)

// ConfigParser is the parser for the config file.
type ConfigParser struct{}

// GetDefaultConfig returns the default config for the application.
func (parser ConfigParser) GetDefaultConfig() config {
	return config{
		Settings: SettingsConfig{
			Local:    true,
			Token:    "",
			DeviceID: "",
			AutoSync: AutoSync{
				BeforeOpen: Sync{
					Remote:  true,
					Locally: true,
				},
				BeforeCreate: Sync{
					Remote:  true,
					Locally: true,
				},
				BeforeUpdate: Sync{
					Remote:  true,
					Locally: true,
				},
				BeforeDelete: Sync{
					Remote:  true,
					Locally: true,
				},
			},
		},
	}
}

// getDefaultConfigYamlContents returns the default config file contents.
func (parser ConfigParser) getDefaultConfigYamlContents() string {
	defaultConfig := parser.GetDefaultConfig()
	yaml, _ := yaml.Marshal(defaultConfig)

	return string(yaml)
}

// writeDefaultConfigContents writes the default config file contents to the given file.
func (parser ConfigParser) writeDefaultConfigContents(newConfigFile *os.File) error {
	_, err := newConfigFile.WriteString(parser.getDefaultConfigYamlContents())

	if err != nil {
		return err
	}

	return nil
}

// createConfigFileIfMissing creates the config file if it doesn't exist.
func (parser ConfigParser) createConfigFileIfMissing(configFilePath string) error {
	if _, err := os.Stat(configFilePath); os.IsNotExist(err) {
		newConfigFile, err := os.OpenFile(configFilePath, os.O_RDWR|os.O_CREATE|os.O_EXCL, 0666)
		if err != nil {
			return err
		}

		defer newConfigFile.Close()
		return parser.writeDefaultConfigContents(newConfigFile)
	}

	return nil
}

// getConfigFileOrCreateIfMissing returns the config file path or creates the config file if it doesn't exist.
func (parser ConfigParser) getConfigFileOrCreateIfMissing() (*string, error) {
	configDir, err := os.UserHomeDir()
	if err != nil {
		return nil, configError{parser: parser, configDir: configDir, err: err}
	}

	prsConfigDir := filepath.Join(configDir, APP_DIR)
	err = os.MkdirAll(prsConfigDir, os.ModePerm)
	if err != nil {
		return nil, configError{parser: parser, configDir: configDir, err: err}
	}

	configFilePath := filepath.Join(prsConfigDir, CONFIG_FILE_NAME)
	err = parser.createConfigFileIfMissing(configFilePath)
	if err != nil {
		return nil, configError{parser: parser, configDir: configDir, err: err}
	}

	return &configFilePath, nil
}

// readConfigFile reads the config file and returns the config.
func (parser ConfigParser) readConfigFile(path string) (config, error) {
	config := parser.GetDefaultConfig()
	data, err := os.ReadFile(path)
	if err != nil {
		return config, configError{parser: parser, configDir: path, err: err}
	}

	err = yaml.Unmarshal((data), &config)
	return config, err
}

// UpdateConfig updates the application's configuration in memory and syncs it to the file.
func UpdateConfig(newConfig config) error {
	parser := initParser()

	configFilePath, err := parser.getConfigFileOrCreateIfMissing()
	if err != nil {
		return err
	}

	yamlData, err := yaml.Marshal(newConfig)
	if err != nil {
		return err
	}

	err = os.WriteFile(*configFilePath, yamlData, 0644)
	if err != nil {
		return err
	}

	Config = newConfig

	return nil
}

// initParser initializes the parser.
func initParser() ConfigParser {
	return ConfigParser{}
}

// ParseConfig parses the config file and returns the config.
func ParseConfig() error {
	var err error

	parser := initParser()

	configFilePath, err := parser.getConfigFileOrCreateIfMissing()
	if err != nil {
		return parsingError{err: err}
	}

	Config, err = parser.readConfigFile(*configFilePath)
	if err != nil {
		return parsingError{err: err}
	}

	return nil
}
