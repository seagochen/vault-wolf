//
// Created by user on 5/9/25.
//

#ifndef VAULTWOLF_VAULT_UTILS_H
#define VAULTWOLF_VAULT_UTILS_H

#include <vector>
#include <cstdint>
#include <string>
#include "CommonDefs.h"

class Utils {

public:
    static inline bool is_base64(std::uint8_t c);
    static std::vector<std::uint8_t> base64_decode(std::string const&);

    static std::string doubleMaxString(double d, std::string def);
    static std::string doubleMaxString(double d);
    static std::string intMaxString(int value);
    static std::string longMaxString(long value);
    static std::string llongMaxString(long long value);

    static std::string getFundDistributionPolicyIndicatorName(FundDistributionPolicyIndicator fundDistributionPolicyIndicator);
    static std::string getFundAssetTypeName(FundAssetType fundAssetType);
};

#endif //VAULTWOLF_VAULT_UTILS_H
