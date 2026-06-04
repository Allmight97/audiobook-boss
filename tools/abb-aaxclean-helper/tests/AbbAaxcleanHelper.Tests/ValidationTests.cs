using System.Text.Json;
using AbbAaxcleanHelper;
using Xunit;

namespace AbbAaxcleanHelper.Tests;

public sealed class ValidationTests
{
    [Fact]
    public void AaxRequiresActivationBytesOnly()
    {
        var request = new MaterializeRequest(
            Protocol.SchemaVersion,
            "op-1",
            MaterializeLane.Aax,
            "/tmp/source.aax",
            "/tmp/output.m4b.partial",
            new MaterializeSecret("0a1b2c3d", null, null)
        );

        Assert.Equal(string.Empty, Validation.Validate(request));
    }

    [Fact]
    public void AaxcRequiresKeyAndIvOnly()
    {
        var request = new MaterializeRequest(
            Protocol.SchemaVersion,
            "op-1",
            MaterializeLane.Aaxc,
            "/tmp/source.aaxc",
            "/tmp/output.m4b.partial",
            new MaterializeSecret(null, "0a0b0c0d0e0f1a1b1c1d1e1f2a2b2c2d", "2e2f3a3b3c3d3e3f4a4b4c4d4e4f5a5b")
        );

        Assert.Equal(string.Empty, Validation.Validate(request));
    }

    [Fact]
    public void MissingSecretIsInvalidRequest()
    {
        var request = new MaterializeRequest(
            Protocol.SchemaVersion,
            "op-1",
            MaterializeLane.Aax,
            "/tmp/source.aax",
            "/tmp/output.m4b.partial",
            null!
        );

        Assert.Equal("Secret payload is required.", Validation.Validate(request));
    }

    [Fact]
    public void ErrorSerializationDoesNotRequireSecretPayloads()
    {
        var message = new ErrorMessage("op-1", "materialization_failed", "AAXClean helper failed during materialization.");

        var json = JsonSerializer.Serialize(message, message.GetType(), Protocol.JsonOptions);

        Assert.Contains("materialization_failed", json);
        Assert.DoesNotContain("0a1b2c3d", json);
        Assert.DoesNotContain("license", json, StringComparison.OrdinalIgnoreCase);
    }

    [Theory]
    [InlineData("input_open_failed")]
    [InlineData("output_open_failed")]
    [InlineData("aax_parse_failed")]
    [InlineData("conversion_failed")]
    [InlineData("invalid_request")]
    public void SafeErrorCategoriesAreSecretFree(string category)
    {
        var message = new ErrorMessage("op-1", category, "AAXClean helper returned a safe diagnostic.");

        var json = JsonSerializer.Serialize(message, message.GetType(), Protocol.JsonOptions);

        Assert.Contains(category, json);
        Assert.DoesNotContain("0a1b2c3d", json);
        Assert.DoesNotContain("token", json, StringComparison.OrdinalIgnoreCase);
        Assert.DoesNotContain("voucher", json, StringComparison.OrdinalIgnoreCase);
    }
}
