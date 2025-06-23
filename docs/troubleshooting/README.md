# Troubleshooting Documentation

This section contains solutions for common issues and error scenarios.

## 🌊 Streaming Issues

### [Stream Error Fixes](./stream-error-fixes.md)
Solutions for WebSocket and streaming related problems:
- Connection failures and timeouts
- Data parsing errors
- Performance issues
- TUI display problems

## 🔍 Common Issues

### Terminal Interface Problems
If the TUI interface isn't working:
1. Ensure you're running in a proper terminal (not piped)
2. Check terminal size and capabilities
3. Verify environment variables (`TERM`, etc.)
4. Use alternative commands with `--assets` parameter

### WebSocket Connection Issues
For connectivity problems:
1. Check network connectivity
2. Verify API endpoints are accessible
3. Review firewall and proxy settings
4. Check authentication credentials

### Data Loading Problems
When datasets or selections don't load:
1. Verify data directory structure
2. Check file permissions
3. Validate JSON file formats
4. Review log files for specific errors

## 🆘 Getting Help

If you can't find a solution here:

1. **Check the logs** - Look in `data/logs/` for detailed error information
2. **Review features docs** - Check [features documentation](../features/) for usage examples
3. **Consult development guides** - See [development documentation](../development/) for testing procedures

## 🔗 Related Documentation

- **[Features](../features/)** - Feature-specific documentation
- **[Development](../development/)** - Testing and development guides
- **[Architecture](../architecture/)** - System design information

---

[← Back to Documentation Index](../README.md)