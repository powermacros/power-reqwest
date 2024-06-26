use power_reqwest::reqwest;

reqwest! {
    name: AliyunSmsClient,
    params: {
        // access point refers to
        // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-endpoint
        api?: String = "dysmsapi",
        // aliyuncs access key
        ak: String,
        // aliyuncs secret key
        sk: String
    },
    hooks: {
        // fill your hook fn names here
        // modify request before submit.
        on_submit: patch_before_submit,
    },
    templates: {
        common_request {
            // https://help.aliyun.com/zh/sdk/product-overview/rpc-mechanism
            // https://next.api.aliyun.com/product/Dysmsapi
            Action: string,
            Version: "2017-05-25",
            Format: "JSON",
            AccessKeyId: string = $$ak,
            SignatureNonce: string = $$sk,
            Timestamp: datetime("%y-%m-%dT%H:%M:%SZ") = default(),
            SignatureMethod: "HMAC-SHA1",
            SignatureVersion: "1.0",
            Signature: string = default(),
        },
    }

    // sign management
    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-dir-signature-management

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-addsmssign
    post add_sms_sign("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "AddSmsSign",
        }
        urlencoded {
            SignName: string = $sign_name,
            SignSource: uint(0..=5) = $sign_source,
            SignFileList {
                FileContents: string,
                FileSuffix: string,
            }[] = $sign_filelist,
            Remark: string = $remark,
            SignType?: uint(0,1) = $sign_type
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            SignName: string,
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-deletesmssign
    post delete_sms_sign("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "DeleteSmsSign",
        }
        urlencoded {
            SignName: string = $sign_name
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            SignName: string,
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-modifysmssign
    post modify_sms_sign("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "ModifySmsSign",
        }
        urlencoded {
            SignName: string = $sign_name,
            SignSource: uint(0..=5) = $sign_source,
            SignFileList {
                FileContents: string,
                FileSuffix: string,
            }[] = $sign_filelist,
            Remark: string = $remark,
            SignType?: uint(0,1) = $sign_type
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            SignName: string,
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-querysmssignlist
    post query_sms_sign_list("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "QuerySmsSignList",
            PageIndex?: uint(1..) = $page_index,
            PageSize?: uint(1..=50) = $page_size
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            TotalCount: uint,
            CurrentPage: uint,
            PageSize: uint,
            SmsSignList {
                OrderId: string,
                SignName: string,
                AuditStatus: string,
                CreateDate: datetime("%y-%m-%d %H:%M:%S"),
                Reason {
                    RejectInfo: string,
                    RejectSubInfo: string,
                    RejectDate: datetime("%y-%m-%d %H:%M:%S"),
                },
                BusinessType: string,
            }[] -> records
        }
    }

    post query_sms_sign("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "QuerySmsSign",
        }
        urlencoded {
            SignName: string = $sign_name
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            OrderId: string,
            SignName: string,
            AuditStatus: string,
            CreateDate: datetime("%y-%m-%d %H:%M:%S"),
            Reason {
                RejectInfo: string,
                RejectSubInfo: string,
                RejectDate: datetime("%y-%m-%d %H:%M:%S"),
            },
            BusinessType: string,
        }
    }

    // template management
    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-dir-template-management

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-addsmstemplate
    post add_sms_template("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "AddSmsTemplate",
        }
        urlencoded {
            TemplateType: uint(0..=3) = $template_type,
            TemplateName: string = $template_name,
            TemplateContent: string = $template_content,
            Remark: string = $remark,
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            TemplateCode: string
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-deletesmstemplate
    post delete_sms_template("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "DeleteSmsTemplate",
        }
        urlencoded {
            TemplateCode: string = $template_code,
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            TemplateCode: string
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-modifysmstemplate
    post modify_sms_template("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "ModifySmsTemplate",
        }
        urlencoded {
            TemplateCode: string = $template_code,
            TemplateType: uint(0..=3) = $template_type,
            TemplateName: string = $template_name,
            TemplateContent: string = $template_content,
            Remark: string = $remark,
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            TemplateCode: string
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-querysmstemplatelist
    post query_sms_template_list("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "QuerySmsTemplateList",
        }
        urlencoded {
            PageIndex?: uint(1..) = $page_index,
            PageSize?: uint(1..=50) = $page_size
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            TotalCount: uint,
            CurrentPage: uint,
            PageSize: uint,
            SmsTemplateList {
                OrderId: string,
                TemplateCode: string,
                TemplateName: string,
                OuterTemplateType: uint(0..=4),
                TemplateType: uint(0..=4),
                AuditStatus: string,
                TemplateContent: string,
                CreateDate: datetime("%y-%m-%d %H:%M:%S"),
                Reason {
                    RejectInfo: string,
                    RejectSubInfo: string,
                    RejectDate: datetime("%y-%m-%d %H:%M:%S"),
                },
            }[] -> records
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-querysmstemplate
    post query_sms_template("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "QuerySmsTemplate",
        }
        urlencoded {
            TemplateCode: string = $template_code,
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            OrderId: string,
            TemplateCode: string,
            TemplateName: string,
            OuterTemplateType: uint(0..=4),
            TemplateType: uint(0..=4),
            AuditStatus: string,
            TemplateContent: string,
            CreateDate: datetime("%y-%m-%d %H:%M:%S"),
            Reason {
                RejectInfo: string,
                RejectSubInfo: string,
                RejectDate: datetime("%y-%m-%d %H:%M:%S"),
            },
        }
    }

    // send sms
    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-dir-send-sms

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-sendsms
    post send_sms("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "SendSms",
        }
        urlencoded {
            PhoneNumbers: string = join_string($phone_numbers: string[], ","),
            SignName: string = $sign_name,
            TemplateCode: string = $template_code,
            TemplateParam: string = json($template_param: object),
            SmsUpExtendCode: string = $sms_up_extend_code,
            OutId: string = $out_id
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            BizId: string,
        }
    }

    post send_batch_sms("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "SendBatchSms",
        }
        urlencoded {
            PhoneNumberJson: string = json($phone_numbers: string[]),
            SignNameJson: json(string[]) = $sign_names,
            TemplateCode: string = $template_code,
            TemplateParamJson: string = json($template_params: object[]),
            SmsUpExtendCodeJson: string = json($sms_up_extend_codes: string[]),
            OutId: string = $out_id
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            BizId: string,
        }
    }


    // query sms sent
    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-dir-send-query

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-querysenddetails
    post query_send_detail("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "QuerySendDetails",
        }
        urlencoded {
            PhoneNumber: string = $phone_number,
            BizId: string = $biz_id,
            SendDate: datetime("%y%m%d") = $send_date,
            PageSize?: uint(1..=50) = $page_size,
            CurrentPage?: uint(1..) = $current_page
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            TotalCount: uint,
            CurrentPage: uint,
            PageSize: uint,
            SmsSendDetailDTOs {
                ErrCode: string,
                TemplateCode: string,
                OutId: string,
                ReceiveDate: string,
                SendDate: string,
                PhoneNum: string,
                Content: string,
                SendStatus: string,
            } -> records
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-querysendstatistics
    post query_send_statistics("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "QuerySendStatistics",
        }
        urlencoded {
            IsGlobe: uint(1,2) = $globe,
            StartDate: string = datetime($start_date, "%y%m%d"),
            EndDate: string = datetime($end_date, "%y%m%d"),
            PageIndex: uint(1..) = $page_index,
            PageSize: uint(1..=50) = $page_size,
            TemplateType: uint(0,1,2,3,7) = $template_type,
            SignName: string = $sign_name,
        }
    } -> {
        json {
            Code: string,
            Message: string,
            RequestId: string,
            TotalSize: uint,
            TargetList {
                TotalCount: uint,
                RespondedSuccessCount: uint,
                RespondedFailCount: uint,
                NoRespondedCount: uint,
                SendDate: datetime("%y%m%d")
            }
        }
    }

    // card template
    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-dir-card-sms

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-getossinfoforcardtemplate
    post get_oss_info_for_card_template("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "GetOSSInfoForCardTemplate"
        }
        urlencoded {
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                Signature: string,
                Host: string,
                Policy: string,
                ExpireTime: string,
                AliUid: string,
                AccessKeyId: string,
                StartPath: string,
                Bucket: string
            }
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-getmediaresourceid
    post get_media_resource_id("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "GetMediaResourceId",
        }
        urlencoded {
            ResourceType: uint(1,2,3,4) = $resource_type,
            OssKey: string = $oss_key,
            FileSize: uint = $file_size,
            ExtendInfo: string = json($extend_info: object),
            Memo: string = $memo,
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                ResourceId: uint,
                ResUrlDownload: string
            }
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-createcardsmstemplate
    post create_card_sms_template("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "CreateCardSmsTemplate",
        }
        urlencoded {
            TemplateName: string = $template_name,
            // template: https://help.aliyun.com/zh/sms/parameters-of-card-sms-templates
            Template {
                extendInfo: {
                    scene: string,
                    purpose: string,
                    params: string,
                    userExt: string
                },
                templateContent: {
                    pages: {
                        tmpCards: {
                            type: string,
                            content: string,
                            srcType: string,
                            src: string,
                            cover: string,
                            actionType: string,
                            positionNumber: string,
                            action: object,
                        }[]
                    }[]
                },
                cardSignName: string,
                cardType: uint,
            } = $template,
            Memo: string = $memo,
            Factorys: string = $factorys
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                TemplateCode: string
            }
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-querycardsmstemplate
    post query_card_sms_template("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "QueryCardSmsTemplate",
        }
        urlencoded {
            TemplateCode: string = $template_code
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                Templates: {
                    tmpName: string,
                    tmpCode: string,
                    state: uint,
                    tmpOps: {
                        tmpOpId: string,
                        vendorTmpId: string,
                        remark: string,
                        supplierCode: string,
                        state: uint,
                        vendorName: string,
                        vendorCode: string,
                    }[],
                }[]
            }
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-checkmobilescardsupport
    post check_mobiles_card_support("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "CheckMobilesCardSupport",
        }
        urlencoded {
            TemplateCode: string = $template_code,
            Mobiles: string = json($mobiles: string[])
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                queryResult {
                    mobile: string,
                    support: bool,
                }[]
            }
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-querymobilescardsupport
    post query_mobiles_card_support("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "QueryMobilesCardSupport",
        }
        urlencoded {
            TemplateCode: string = $template_code,
            Mobiles: string = json($mobiles: string[])
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                QueryResult {
                    mobile: string,
                    support: bool,
                } []
            }
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-getcardsmslink
    post get_card_sms_link("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "GetCardSmsLink",
        }
        urlencoded {
            CardTemplateCode: string = $card_template_code,
            OutId?: string = $out_id,
            PhoneNumberJson?: string = json($phone_numbers: string[]),
            SignNameJson?: string = json($sign_names: string[]),
            CardTemplateParamJson?: string = json($card_template_params: object[]),
            CardCodeType?: uint(1,2) = $card_code_type,
            CardLinkType?: uint(1,2) = $card_link_type,
            Domain?: string = $domain,
            CustomShortCodeJson?: string = $custom_short_code
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                CardTmpState: uint,
                NotMediaMobiles: string,
                CardPhoneNumbers: json(string[]),
                CardSmsLinks: json(string[]),
                CardSignNames: json(string[])
            }
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-querycardsmstemplatereport
    post query_card_sms_template_report("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "QueryCardSmsTemplateReport",
        }
        urlencoded {
            TemplateCodes: string[] = $template_codes,
            StartDate: string = datetime($start_date, "%y-%m-%d %H:%M:%S"),
            EndDate: string = datetime($end_date, "%y-%m-%d %H:%M:%S"),
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                model {
                    tmpCode: string,
                    date: datetime("%y-%m-%d %H:%M:%S"),
                    rptSuccessCount: uint,
                    exposeUv: uint,
                    exposePv: uint,
                    clickUv: uint,
                    clickPv: uint
                }[]
            }
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-sendcardsms
    post send_card_sms("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "SendCardSms",
        }
        urlencoded {
            CardObjects: {
                customUrl?: string,
                dyncParams?: string,
                mobile?: string
            }[] = $card_objects,
            SignName: string = $sign_name,
            CardTemplateCode: string = $card_template_code,
            SmsTemplateCode: string = $sms_template_code,
            SmsUpExtendCode: string = $sms_up_extend_code,
            FallbackType: string = $fallback_type,
            DigitalTemplateCode?: string = $digital_template_code,
            OutId?: string = $out_id,
            SmsTemplateParam?: json(object) = $sms_template_param,
            DigitalTemplateParam?: json(object) =$digital_template_param,
            TemplateCode?: string = $template_code,
            TemplateParam?: json(object) = $template_param,
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                MediaMobiles: string,
                BizCardId: string,
                BizDigitalId: string,
                CardTmpState: uint,
                NotMediaMobiles: string,
                BizSmsId: string,
            }
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-sendbatchcardsms
    post send_batch_card_sms("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "SendBatchCardSms",
        }
        urlencoded {
            CardTemplateCode: string = $card_template_code,
            SmsTemplateCode: string = $sms_template_code,
            FallbackType: string = $fallback_type,
            DigitalTemplateCode?: string = $digital_template_code,
            OutId?: string = $out_id,
            PhoneNumberJson: string = json($phone_numbers: string[]),
            SignNameJson: string = json($sign_names: string[]),
            CardTemplateParamJson?: json(object[]) = $card_template_params,
            SmsTemplateParamJson?: json(object[]) = $sms_template_params,
            DigitalTemplateParamJson?: json(object[]) = $digital_template_params,
            SmsUpExtendCodeJson?: string = json($sms_up_extend_codes: string[]),
            TemplateCode?: string = $template_code,
            TemplateParamJson?: string = json($template_params: object[]),
        }
    } -> {
        json {
            Success: bool,
            RequestId: string,
            Code: string,
            Data? {
                MediaMobiles: string,
                BizCardId: string,
                BizDigitalId: string,
                CardTmpState: uint,
                NotMediaMobiles: string,
                BizSmsId: string,
            }
        }
    }

    // sms conversion
    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-dir-domestic-and-international-sms-conversion-rate

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-smsconversionintl
    post sms_coversion_intl("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "SmsConversionIntl",
        }
        urlencoded {
            MessageId: string = $message_id,
            Delivered: bool = $delivered,
            ConversionTime?: uint = timestamp($conversion_time)
        }
    } -> {
        json: {
            Code: string,
            Message: string,
            RequestId: string,
        }
    }


    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-conversiondataintl
    post conversion_data_intl("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "ConversionDataIntl",
        }
        urlencoded {
            ReportTime?: uint = timestamp($report_time),
            ConversionRate: string = format("{}", $conversion_rate: float)
        }
    } -> {
        json: {
            Code: string,
            Message: string,
            RequestId: string,
        }
    }

    // resources tag magement
    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-dir-label-management

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-listtagresources
    post list_tag_resources("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "ListTagResources",
        }
        urlencoded {
            ResourceType: string = $resource_type || "TEMPLATE",
            RegionId: string = $region_id,
            NextToken?: string = $next_token,
            PageSize?: uint(1..) = $page_size,
            ProdCode?: string = $prod_code || "dysms",
            Tag {
                Key: string,
                Value: string,
            }[] = $tag,
            ResourceId?: string[] = $resource_id,
        }
    } -> {
        json {
            Code: string,
            NextToken: string,
            RequestId: string,
            TagResources {
                ResourceType: string,
                TagValue: string,
                ResourceId: string,
                TagKey: string,
            }[]
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-tagresources
    post tag_resources("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "TagResources",
        }
        urlencoded {
            ResourceType: string = $resource_type || "TEMPLATE",
            RegionId: string = $region_id,
            ProdCode?: string = $prod_code || "dysms",
            Tag: {
                Key: string,
                Value: string,
            }[] = $tag,
            ResourceId?: string[] = $resource_id,
        }
    } -> {
        json {
            Code: string,
            RequestId: string,
            Data: string
        }
    }

    // https://help.aliyun.com/zh/sms/developer-reference/api-dysmsapi-2017-05-25-untagresources
   post untag_resources("https://$$api.aliyuncs.com") {
        query: common_request {
            Action: "UntagResources",
        }
        urlencoded {
            ResourceType: string = $resource_type || "TEMPLATE",
            RegionId: string = $region_id,
            All: bool = $all,
            ProdCode?: string = $prod_code || "dysms",
            TagKey?: string [] = $tag_key,
            ResourceId?: string[] = $resource_id,
        }
    } -> {
        json {
            Code: string,
            RequestId: string,
            Data: string
        }
    }
}

fn main() {}
